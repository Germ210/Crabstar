use std::sync::{Arc, Mutex};

use crate::world::{CrabstarDocument, World};
use crabstar_frontend::{
  ast::{AstNode, Root},
  err::Reason,
  module::Module,
  parser::{Parser, parse},
  syntax::SyntaxNode,
};
use ropey::Rope;
use tower_lsp_server::{
  Client, LanguageServer,
  jsonrpc::Result,
  ls_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    InitializeParams, InitializeResult, InitializedParams, MessageType, Position, Range,
    SemanticTokens, SemanticTokensFullOptions, SemanticTokensLegend, SemanticTokensOptions,
    SemanticTokensParams, SemanticTokensResult, SemanticTokensServerCapabilities,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, Uri,
  },
};
use tracing::info;

use crate::diagnostics::type_error_to_diagnostic;

#[derive(Debug)]
pub struct Backend {
  pub client: Client,
  pub world: World,
}

fn get_type_diagnostics(world: &World, uri: &Uri, rope: &Rope, root: Root) -> Vec<Diagnostic> {
  let module_arc = {
    let entry = world.open_docs.get(uri).expect("Document is not open");
    entry.module.clone()
  };

  let mut module = module_arc.lock().unwrap();
  *module = Module::new();
  module.infer_module(root);

  let errs = std::mem::take(&mut module.errs);

  errs
    .iter()
    .map(|e| type_error_to_diagnostic(e, rope, &mut module.inferencer))
    .collect::<Vec<_>>()
}

async fn run_inference(client: &Client, world: &World, uri: &Uri, version: Option<i32>) {
  let (source, rope) = {
    let entry = world.open_docs.get(uri).expect("Document is not open");
    (entry.source.to_string(), entry.source.clone())
  };

  let mut parser = Parser::new(&source);
  parse(&mut parser);
  let parse_diagnostics: Vec<Diagnostic> = parser
    .errs
    .iter()
    .map(|e| {
      let start = char_index_to_position(&rope, e.span.start);
      let end = char_index_to_position(&rope, e.span.end);
      Diagnostic {
        range: Range { start, end },
        severity: Some(DiagnosticSeverity::ERROR),
        message: match e.reason.as_ref() {
          Reason::Custom(msg) => msg.clone(),
          Reason::Expected { found, expected } => {
            format!("Expected {:?}, found {:?}", expected, found)
          }
        },
        ..Default::default()
      }
    })
    .collect();

  let green = parser.build_tree();
  let Some(root) = Root::cast(SyntaxNode::new_root(green)) else {
    info!("diagnostics: {:?}", parse_diagnostics);
    client
      .publish_diagnostics(uri.clone(), parse_diagnostics, version)
      .await;
    return;
  };

  let type_diagnostics = get_type_diagnostics(world, uri, &rope, root);

  let mut diagnostics = parse_diagnostics;
  diagnostics.extend(type_diagnostics);

  info!("diagnostics: {:?}", diagnostics);
  client
    .publish_diagnostics(uri.clone(), diagnostics, version)
    .await;
}

impl LanguageServer for Backend {
  async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
    Ok(InitializeResult {
      capabilities: ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(
          TextDocumentSyncKind::INCREMENTAL,
        )),
        semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
          SemanticTokensOptions {
            legend: SemanticTokensLegend {
              token_types: vec![],
              token_modifiers: vec![],
            },
            full: Some(SemanticTokensFullOptions::Bool(true)),
            ..Default::default()
          },
        )),
        ..Default::default()
      },
      ..Default::default()
    })
  }

  async fn initialized(&self, _: InitializedParams) {
    self
      .client
      .log_message(MessageType::INFO, "server initialized!")
      .await;
  }

  async fn shutdown(&self) -> Result<()> {
    Ok(())
  }

  async fn did_open(&self, params: DidOpenTextDocumentParams) {
    let document = params.text_document;
    info!("did_open: {:?}", document.uri);
    self.world.open_docs.insert(
      document.uri.clone(),
      CrabstarDocument {
        source: Rope::from_str(&document.text),
        module: Arc::new(Mutex::new(Module::new())),
      },
    );
    run_inference(&self.client, &self.world, &document.uri, None).await;
  }

  async fn did_change(&self, params: DidChangeTextDocumentParams) {
    let uri = params.text_document.uri;
    let version = params.text_document.version;
    info!("did_change: {:?} version: {}", uri, version);
    {
      let mut entry = self
        .world
        .open_docs
        .get_mut(&uri)
        .expect("Document is not open");
      for change in params.content_changes {
        let range = change.range.unwrap();
        let start = position_to_char_index(&entry.source, range.start);
        let end = position_to_char_index(&entry.source, range.end);
        entry.source.remove(start..end);
        entry.source.insert(start, &change.text);
        info!("{}", entry.source.to_string());
      }
      drop(entry);
    }
    info!("calling run_inference after edit");
    run_inference(&self.client, &self.world, &uri, None).await;
  }

  async fn semantic_tokens_full(
    &self,
    _: SemanticTokensParams,
  ) -> Result<Option<SemanticTokensResult>> {
    Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
      result_id: None,
      data: vec![],
    })))
  }
}

pub fn position_to_char_index(rope: &Rope, position: Position) -> usize {
  let line_start = rope.line_to_char(position.line as usize);
  let line_start_utf16 = rope.char_to_utf16_cu(line_start);
  rope.utf16_cu_to_char(line_start_utf16 + position.character as usize)
}

pub fn char_index_to_position(rope: &Rope, char_index: usize) -> Position {
  let line = rope.char_to_line(char_index);
  let line_start = rope.line_to_char(line);
  Position {
    line: line as u32,
    character: (rope.char_to_utf16_cu(char_index) - rope.char_to_utf16_cu(line_start)) as u32,
  }
}
