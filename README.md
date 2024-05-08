# tree-sitter-c2rust (with parser compatibility)

This repo allows you link `tree-sitter`, as well as a parser to a Rust WebAssembly project.

## You'll need

- emscripten (emcc)
- mesonbuild
- c2rust
- A unix based environment (This procedure has been tested under a Linux machine)

## Steps

a. Compile the parser you want with `tree-sitter generate`

b. Move the parser from `src/parser.c`, into this library at `lib/src/imported_parser.c`

  1. If The parser has a `scanner.c`, move that into `lib/src/imported_scanner.c`

c. In the `imported_parser.c` file (and in the `imported_scanner.c` file), change all the `tree-sitter` includes to be local

```diff
- #include "tree-sitter/parser.h"
+ #include "parser.h"
```
  1. If you have an `imported_scanner.c`, include that in the parser file

```diff
+ #include "imported_scanner.c"
```
d. Compile a `compile_commands.json`
```shell
meson setup build --buildtype=release --cross-file=wasm.txt --default-library=static
```

e. Run the transpiler
```shell
bash transpile.sh
```

f. Run `cargo check`, there should be a small amount of errors. Fix them

  1. There is one error with an easy fix that is consistent.
  ```
  error[E0425]: cannot find value `run_static_initializers` in this scope
        --> lib/binding_rust/core_wrapper/core/api_raw.rs:303148:51
         |
  303148 | static INIT_ARRAY: [unsafe extern "C" fn(); 1] = [run_static_initializers];
         |                                                   ^^^^^^^^^^^^^^^^^^^^^^^ not found in this scope
         |
  ```
  In both `api_raw.rs` and `imported_parser.rs`, delete this `INIT_ARRAY` and associated attributes above it. Under `imported_parser.rs`, in the `tree_sitter_xxx()` function, before the `return &language` statement, put `run_static_initializers()`

```diff
pub unsafe extern "C" fn tree_sitter_xxxx() -> *const TSLanguage {
+   run_static_initializers();
    return &language;
}
```

g. Profit! Include new `tree-sitter` in your project
```toml
tree-sitter = { path = "tree-sitter/lib" }
```

Add your language like so, and use `tree-sitter as you normally would!`
```rust
let lang = unsafe { tree_sitter::Language::from_raw(tree_sitter::core::tree_sitter_xxxx()) };
```
