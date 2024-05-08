project('tree_sitter', 'c', default_options: ['c_std=gnu99'])

tree_sitter_lib = library(
  'tree_sitter',
  'lib/src/processed/lib.c',
  'lib/src/processed/alloc.c',
  'lib/src/processed/get_changed_ranges.c',
  'lib/src/processed/language.c',
  'lib/src/processed/lexer.c',
  'lib/src/processed/node.c',
  'lib/src/processed/parser.c',
  'lib/src/processed/query.c',
  'lib/src/processed/stack.c',
  'lib/src/processed/subtree.c',
  'lib/src/processed/tree.c',
  'lib/src/processed/tree_cursor.c',
  'lib/src/processed/wasm_store.c',
  'lib/src/processed/imported_parser.c',
  include_directories: ['lib/include', 'lib/src', 'lib/src/wasm']
)

tree_sitter_dep = declare_dependency(
  link_with: [tree_sitter_lib],
  include_directories: ['lib/include']
)
