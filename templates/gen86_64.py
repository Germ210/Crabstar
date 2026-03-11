import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from itertools import product

PHYS_REGS = [
    "rax",
    "rcx",
    "rdx",
    "rsi",
    "rdi",
    "r8",
    "r9",
    "r10",
    "r11",
    "rbx",
    "rbp",
    "r12",
    "r13",
    "r14",
    "r15",
]

REG_TO_PHYS = {
    "rax": "PhysReg::Rax",
    "rcx": "PhysReg::Rcx",
    "rdx": "PhysReg::Rdx",
    "rbx": "PhysReg::Rbx",
    "rsi": "PhysReg::Rsi",
    "rdi": "PhysReg::Rdi",
    "rbp": "PhysReg::Rbp",
    "r8": "PhysReg::R8",
    "r9": "PhysReg::R9",
    "r10": "PhysReg::R10",
    "r11": "PhysReg::R11",
    "r12": "PhysReg::R12",
    "r13": "PhysReg::R13",
    "r14": "PhysReg::R14",
    "r15": "PhysReg::R15",
}

RELOC_TARGET = "__crabstar_reloc_target"

INSTRUCTIONS = [
    {
        "name": "add",
        "has_reloc": False,
        "params": ["INPUT0", "INPUT1", "OUTPUT"],
        "ir": """
define void @fn() naked noreturn {
  %a = call i64 asm sideeffect "", "={INPUT0}"()
  %b = call i64 asm sideeffect "", "={INPUT1}"()
  %sum = add i64 %a, %b
  call void asm sideeffect "", "{OUTPUT}"(i64 %sum)
  unreachable
}
""",
    },
    {
        "name": "sub",
        "has_reloc": False,
        "params": ["INPUT0", "INPUT1", "OUTPUT"],
        "ir": """
define void @fn() naked noreturn {
  %a = call i64 asm sideeffect "", "={INPUT0}"()
  %b = call i64 asm sideeffect "", "={INPUT1}"()
  %r = sub i64 %a, %b
  call void asm sideeffect "", "{OUTPUT}"(i64 %r)
  unreachable
}
""",
    },
    {
        "name": "imul",
        "has_reloc": False,
        "params": ["INPUT0", "INPUT1", "OUTPUT"],
        "ir": """
define void @fn() naked noreturn {
  %a = call i64 asm sideeffect "", "={INPUT0}"()
  %b = call i64 asm sideeffect "", "={INPUT1}"()
  %r = mul i64 %a, %b
  call void asm sideeffect "", "{OUTPUT}"(i64 %r)
  unreachable
}
""",
    },
    {
        "name": "idiv",
        "has_reloc": False,
        "params": ["INPUT0"],
        "ir": """
define void @fn() naked noreturn {
  %a = call i64 asm sideeffect "", "={INPUT0}"()
  call void asm sideeffect "cqo; idivq $0", "r"(i64 %a)
  unreachable
}
""",
    },
    {
        "name": "neg",
        "has_reloc": False,
        "params": ["INPUT0", "OUTPUT"],
        "ir": """
define void @fn() naked noreturn {
  %a = call i64 asm sideeffect "", "={INPUT0}"()
  %r = sub i64 0, %a
  call void asm sideeffect "", "{OUTPUT}"(i64 %r)
  unreachable
}
""",
    },
    {
        "name": "not",
        "has_reloc": False,
        "params": ["INPUT0", "OUTPUT"],
        "ir": """
define void @fn() naked noreturn {
  %a = call i64 asm sideeffect "", "={INPUT0}"()
  %r = xor i64 %a, -1
  call void asm sideeffect "", "{OUTPUT}"(i64 %r)
  unreachable
}
""",
    },
    {
        "name": "mov",
        "has_reloc": False,
        "params": ["INPUT0", "OUTPUT"],
        "ir": """
define void @fn() naked noreturn {
  %a = call i64 asm sideeffect "", "={INPUT0}"()
  call void asm sideeffect "", "{OUTPUT}"(i64 %a)
  unreachable
}
""",
    },
    {
        "name": "movabs",
        "has_reloc": True,
        "params": ["OUTPUT"],
        "ir": f"""
@{RELOC_TARGET} = external global i64
define void @fn() naked noreturn {{
  %v = call i64 asm sideeffect "movabsq $${RELOC_TARGET}, $0", "=r"()
  call void asm sideeffect "", "{{OUTPUT}}"(i64 %v)
  unreachable
}}
""",
    },
    {
        "name": "cmp",
        "has_reloc": False,
        "params": ["INPUT0", "INPUT1"],
        "ir": """
define void @fn() naked noreturn {
  %a = call i64 asm sideeffect "", "={INPUT0}"()
  %b = call i64 asm sideeffect "", "={INPUT1}"()
  call void asm sideeffect "cmpq $1, $0", "r,r"(i64 %a, i64 %b)
  unreachable
}
""",
    },
    {
        "name": "sete",
        "has_reloc": False,
        "params": ["OUTPUT"],
        "ir": """
define void @fn() naked noreturn {
  %a = call i8 asm sideeffect "sete ${0:b}", "=r"()
  %b = zext i8 %a to i64
  call void asm sideeffect "", "{OUTPUT}"(i64 %b)
  unreachable
}
""",
    },
    {
        "name": "setne",
        "has_reloc": False,
        "params": ["OUTPUT"],
        "ir": """
define void @fn() naked noreturn {
  %a = call i8 asm sideeffect "setne ${0:b}", "=r"()
  %b = zext i8 %a to i64
  call void asm sideeffect "", "{OUTPUT}"(i64 %b)
  unreachable
}
""",
    },
    {
        "name": "setl",
        "has_reloc": False,
        "params": ["OUTPUT"],
        "ir": """
define void @fn() naked noreturn {
  %a = call i8 asm sideeffect "setl ${0:b}", "=r"()
  %b = zext i8 %a to i64
  call void asm sideeffect "", "{OUTPUT}"(i64 %b)
  unreachable
}
""",
    },
    {
        "name": "setle",
        "has_reloc": False,
        "params": ["OUTPUT"],
        "ir": """
define void @fn() naked noreturn {
  %a = call i8 asm sideeffect "setle ${0:b}", "=r"()
  %b = zext i8 %a to i64
  call void asm sideeffect "", "{OUTPUT}"(i64 %b)
  unreachable
}
""",
    },
    {
        "name": "setg",
        "has_reloc": False,
        "params": ["OUTPUT"],
        "ir": """
define void @fn() naked noreturn {
  %a = call i8 asm sideeffect "setg ${0:b}", "=r"()
  %b = zext i8 %a to i64
  call void asm sideeffect "", "{OUTPUT}"(i64 %b)
  unreachable
}
""",
    },
    {
        "name": "setge",
        "has_reloc": False,
        "params": ["OUTPUT"],
        "ir": """
define void @fn() naked noreturn {
  %a = call i8 asm sideeffect "setge ${0:b}", "=r"()
  %b = zext i8 %a to i64
  call void asm sideeffect "", "{OUTPUT}"(i64 %b)
  unreachable
}
""",
    },
    {
        "name": "ret",
        "has_reloc": False,
        "params": [],
        "ir": """
define void @fn() naked noreturn {
  call void asm sideeffect "retq", ""()
  unreachable
}
""",
    },
    {
        "name": "jmp_rel32",
        "has_reloc": True,
        "params": [],
        "ir": f"""
declare void @{RELOC_TARGET}()
define void @fn() naked noreturn {{
  call void asm sideeffect "jmp {RELOC_TARGET}", ""()
  unreachable
}}
""",
    },
    {
        "name": "je_rel32",
        "has_reloc": True,
        "params": [],
        "ir": f"""
declare void @{RELOC_TARGET}()
define void @fn() naked noreturn {{
  call void asm sideeffect "je {RELOC_TARGET}", ""()
  unreachable
}}
""",
    },
    {
        "name": "jne_rel32",
        "has_reloc": True,
        "params": [],
        "ir": f"""
declare void @{RELOC_TARGET}()
define void @fn() naked noreturn {{
  call void asm sideeffect "jne {RELOC_TARGET}", ""()
  unreachable
}}
""",
    },
    {
        "name": "jl_rel32",
        "has_reloc": True,
        "params": [],
        "ir": f"""
declare void @{RELOC_TARGET}()
define void @fn() naked noreturn {{
  call void asm sideeffect "jl {RELOC_TARGET}", ""()
  unreachable
}}
""",
    },
    {
        "name": "jle_rel32",
        "has_reloc": True,
        "params": [],
        "ir": f"""
declare void @{RELOC_TARGET}()
define void @fn() naked noreturn {{
  call void asm sideeffect "jle {RELOC_TARGET}", ""()
  unreachable
}}
""",
    },
    {
        "name": "jg_rel32",
        "has_reloc": True,
        "params": [],
        "ir": f"""
declare void @{RELOC_TARGET}()
define void @fn() naked noreturn {{
  call void asm sideeffect "jg {RELOC_TARGET}", ""()
  unreachable
}}
""",
    },
    {
        "name": "jge_rel32",
        "has_reloc": True,
        "params": [],
        "ir": f"""
declare void @{RELOC_TARGET}()
define void @fn() naked noreturn {{
  call void asm sideeffect "jge {RELOC_TARGET}", ""()
  unreachable
}}
""",
    },
]

CACHE_FILE = ".template_cache.json"


def load_cache():
    if os.path.exists(CACHE_FILE):
        with open(CACHE_FILE, "r") as f:
            return json.load(f)
    return {}


def save_cache(cache):
    with open(CACHE_FILE, "w") as f:
        json.dump(cache, f)


def cache_key(ir):
    return hashlib.sha256(ir.encode()).hexdigest()


def get_relocations(obj_path):
    r = subprocess.run(
        ["llvm-readobj", "--relocations", obj_path],
        capture_output=True,
        check=True,
    )
    output = r.stdout.decode()
    relocs = []
    for line in output.splitlines():
        m = re.search(r"0x([0-9a-fA-F]+)\s+R_X86_64_(\w+)", line)
        if not m:
            m = re.search(r"0x([0-9a-fA-F]+)\s+IMAGE_REL_AMD64_(\w+)", line)
        if m:
            offset = int(m.group(1), 16)
            kind = m.group(2)
            size = {
                "PC32": 4,
                "PLT32": 4,
                "PC8": 1,
                "64": 8,
                "32": 4,
                "32S": 4,
                "REL32": 4,
                "REL32_1": 4,
                "REL32_2": 4,
                "ADDR32NB": 4,
                "ADDR64": 8,
            }.get(kind, 4)
            relocs.append((offset, size))
    return relocs


def compile_ir(ir):
    with tempfile.TemporaryDirectory() as tmp:
        ll_path = os.path.join(tmp, "t.ll")
        obj_path = os.path.join(tmp, "t.o")
        bin_path = os.path.join(tmp, "t.bin")
        with open(ll_path, "w") as f:
            f.write(ir)
        r = subprocess.run(
            [
                "clang",
                "-O2",
                "-c",
                "-target",
                "x86_64-pc-linux-gnu",
                ll_path,
                "-o",
                obj_path,
            ],
            capture_output=True,
        )
        if r.returncode != 0:
            raise RuntimeError(f"clang failed:\n{r.stderr.decode()}")
        relocs = get_relocations(obj_path)
        subprocess.run(
            ["llvm-objcopy", "--dump-section", f".text={bin_path}", obj_path],
            capture_output=True,
            check=True,
        )
        with open(bin_path, "rb") as f:
            return f.read(), relocs


def compile_combo(args):
    ir_template, regs, params, cache = args
    ir = ir_template
    for param, reg in zip(params, regs):
        ir = ir.replace(param, reg)
    key = cache_key(ir)
    if key in cache:
        cached = cache[key]
        if cached is None:
            return regs, None, []
        return regs, bytes(cached["bytes"]), [tuple(r) for r in cached["relocs"]]
    try:
        result, relocs = compile_ir(ir)
        cache[key] = {"bytes": list(result), "relocs": [list(r) for r in relocs]}
        return regs, result, relocs
    except RuntimeError as e:
        print(f"compile error for {regs}: {e}", file=sys.stderr)
        cache[key] = None
        return regs, None, []


def zero_reloc_bytes(b, relocs):
    b = bytearray(b)
    for offset, size in relocs:
        for i in range(offset, min(offset + size, len(b))):
            b[i] = 0
    return bytes(b)


def process_instruction(instr, cache):
    name = instr["name"]
    params = instr["params"]
    ir_template = instr["ir"]
    has_relocs = instr["has_reloc"]

    if params:
        all_combos = list(product(PHYS_REGS, repeat=len(params)))
        args_list = [(ir_template, regs, params, cache) for regs in all_combos]
        with ThreadPoolExecutor() as executor:
            results = list(executor.map(compile_combo, args_list))
    else:
        regs, b, relocs = compile_combo((ir_template, (), [], cache))
        if b is None:
            print(f"ERROR: {name} failed to compile, skipping", file=sys.stderr)
            return
        results = [(regs, b, relocs)]

    if has_relocs:
        all_relocs = [r for _, _, r in results if r]
        representative_relocs = all_relocs[0] if all_relocs else []
        patch_offset, patch_size = representative_relocs[0]
        ty = f"i{patch_size * 8}"
        if params:
            fn_params = ", ".join(f"{p.lower()}: PhysReg" for p in params)
            print(f"pub fn emit_{name}(buf: &mut Vec<u8>, {fn_params}, rel: {ty}) {{")
        else:
            print(f"pub fn emit_{name}(buf: &mut Vec<u8>, rel: {ty}) {{")
        print(f"  let base = buf.len();")
        if params:
            print(f"  match ({', '.join(p.lower() for p in params)}) {{")
            for regs, bytes_, relocs in sorted(results, key=lambda x: x[0]):
                if bytes_ is None:
                    continue
                phys = ", ".join(REG_TO_PHYS[r] for r in regs)
                zeroed = zero_reloc_bytes(
                    bytes_, relocs if relocs else representative_relocs
                )
                arr = ", ".join(f"0x{x:02X}" for x in zeroed)
                print(f"    ({phys}) => buf.extend_from_slice(&[{arr}]),")
            print(f"    _ => unreachable!(),")
            print(f"  }}")
        else:
            _, bytes_, relocs = results[0]
            zeroed = zero_reloc_bytes(bytes_, relocs)
            arr = ", ".join(f"0x{x:02X}" for x in zeroed)
            print(f"  buf.extend_from_slice(&[{arr}]);")
        print(f"  let bytes = rel.to_le_bytes();")
        print(
            f"  buf[base + {patch_offset}..base + {patch_offset} + {patch_size}].copy_from_slice(&bytes);"
        )
        print(f"}}")
    else:
        if params:
            fn_params = ", ".join(f"{p.lower()}: PhysReg" for p in params)
            print(f"pub fn emit_{name}(buf: &mut Vec<u8>, {fn_params}) {{")
            print(f"  match ({', '.join(p.lower() for p in params)}) {{")
            for regs, bytes_, _ in sorted(results, key=lambda x: x[0]):
                if bytes_ is None:
                    continue
                phys = ", ".join(REG_TO_PHYS[r] for r in regs)
                arr = ", ".join(f"0x{b:02X}" for b in bytes_)
                print(f"    ({phys}) => buf.extend_from_slice(&[{arr}]),")
            print(f"    _ => unreachable!(),")
            print(f"  }}")
        else:
            _, bytes_, _ = results[0]
            arr = ", ".join(f"0x{b:02X}" for b in bytes_)
            print(f"pub fn emit_{name}(buf: &mut Vec<u8>) {{")
            print(f"  buf.extend_from_slice(&[{arr}]);")
        print(f"}}")
    print()


def main():
    import builtins

    original_print = builtins.print

    if len(sys.argv) > 1:
        out = open(sys.argv[1], "w", encoding="utf-8", newline="\n")

        def print_to_file(*args, **kwargs):
            kwargs.setdefault("file", out)
            original_print(*args, **kwargs)

        builtins.print = print_to_file
    else:
        out = None

    cache = load_cache()

    try:
        print("use crate::regalloc::x86_64::PhysReg;")
        print()
        for instr in INSTRUCTIONS:
            process_instruction(instr, cache)
    finally:
        save_cache(cache)
        builtins.print = original_print
        if out is not None:
            out.close()


if __name__ == "__main__":
    main()
