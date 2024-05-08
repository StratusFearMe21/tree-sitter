use std::collections::HashMap;

use indicatif::ParallelProgressIterator;
use rayon::iter::{ParallelBridge, ParallelIterator};
use syn::{__private::ToTokens, visit_mut::VisitMut, ExprLit};

struct AllocReplacer {}
impl syn::visit_mut::VisitMut for AllocReplacer {
    fn visit_expr_mut(&mut self, i: &mut syn::Expr) {
        match i {
            syn::Expr::MethodCall(m) => {
                if m.method.to_token_stream().to_string() == "expect" {
                    if m.receiver.to_token_stream().to_string() == "ts_current_free" {
                        *i = syn::parse_quote!(crate::core::alloc::ts_free);
                    } else if m.receiver.to_token_stream().to_string() == "ts_current_malloc" {
                        *i = syn::parse_quote!(crate::core::alloc::ts_malloc);
                    } else if m.receiver.to_token_stream().to_string() == "ts_current_calloc" {
                        *i = syn::parse_quote!(crate::core::alloc::ts_calloc);
                    } else if m.receiver.to_token_stream().to_string() == "ts_current_realloc" {
                        *i = syn::parse_quote!(crate::core::alloc::ts_realloc);
                    }
                }
            }
            _ => {}
        }

        syn::visit_mut::visit_expr_mut(self, i);
    }
}

struct MemcpyReplacer {}
impl syn::visit_mut::VisitMut for MemcpyReplacer {
    fn visit_expr_call_mut(&mut self, i: &mut syn::ExprCall) {
        if i.func.to_token_stream().to_string() == "memcpy" {
            let dst = i.args[0].clone();
            let src = i.args[1].clone();
            let size = i.args[2].clone();
            *i = syn::parse_quote!(std::ptr::copy_nonoverlapping(#src, #dst, #size));
        } else if i.func.to_token_stream().to_string() == "memmove" {
            let dst = i.args[0].clone();
            let src = i.args[1].clone();
            let size = i.args[2].clone();
            *i = syn::parse_quote!(std::ptr::copy(#src, #dst, (#size) as usize));
        } else if i.func.to_token_stream().to_string() == "memset" {
            let dst = i.args[0].clone();
            let value = i.args[1].clone();
            let size = i.args[2].clone();
            *i = syn::parse_quote!(std::ptr::write_bytes(#dst, (#value) as u8, (#size) as usize));
        }

        syn::visit_mut::visit_expr_call_mut(self, i);
    }
}

struct MemcmpReplacer {}
impl syn::visit_mut::VisitMut for MemcmpReplacer {
    fn visit_expr_mut(&mut self, i: &mut syn::Expr) {
        if let syn::Expr::Call(c) = i {
            if c.func.to_token_stream().to_string() == "memcmp" {
                let lhs = c.args[0].clone();
                let rhs = c.args[1].clone();
                let size = c.args[2].clone();

                if let syn::Expr::Cast(syn::ExprCast { expr: lhs, .. }) = lhs {
                    if let syn::Expr::Cast(syn::ExprCast { expr: rhs, .. }) = rhs {
                        *i = syn::parse_quote! {
                            if std::slice::from_raw_parts(#lhs as *const u8, #size) == std::slice::from_raw_parts(#rhs as *const u8, #size) {
                                0
                            } else {
                                1
                            }
                        };
                    }
                }
            }
        }

        syn::visit_mut::visit_expr_mut(self, i);
    }
}

struct SnprintfReplacer {}
impl syn::visit_mut::VisitMut for SnprintfReplacer {
    fn visit_expr_mut(&mut self, expr: &mut syn::Expr) {
        if let syn::Expr::Call(i) = expr {
            if let Some((to, limit, pattern, args, is_file)) =
                if i.func.to_token_stream().to_string() == "snprintf" {
                    let to = i.args[0].clone();
                    let limit = i.args[1].clone();
                    let pattern = i.args[2].clone();
                    let args = i.args.iter().skip(3).collect::<Vec<_>>();

                    Some((to, limit, pattern, args, false))
                } else if i.func.to_token_stream().to_string() == "sprintf" {
                    let to = i.args[0].clone();
                    let limit = syn::parse_quote!(1024);
                    let pattern = i.args[1].clone();
                    let args = i.args.iter().skip(2).collect::<Vec<_>>();

                    Some((to, limit, pattern, args, false))
                } else if i.func.to_token_stream().to_string() == "fprintf" {
                    let to = i.args[0].clone();
                    let limit = syn::parse_quote!(1024);
                    let pattern = i.args[1].clone();
                    let args = i.args.iter().skip(2).collect::<Vec<_>>();

                    Some((to, limit, pattern, args, true))
                } else {
                    None
                }
            {
                let (has_condition, condition, pattern1, pattern2) =
                    if let syn::Expr::If(syn::ExprIf {
                        cond,
                        then_branch:
                            syn::Block {
                                stmts: stmts_then, ..
                            },
                        else_branch: Some((_, else_expr)),
                        ..
                    }) = pattern
                    {
                        if let syn::Stmt::Expr(e_then) = &stmts_then[0] {
                            if let syn::Expr::Block(syn::ExprBlock {
                                block:
                                    syn::Block {
                                        stmts: stmts_else, ..
                                    },
                                ..
                            }) = else_expr.as_ref()
                            {
                                if let syn::Stmt::Expr(e_else) = &stmts_else[0] {
                                    (true, cond.as_ref().clone(), e_then.clone(), e_else.clone())
                                } else {
                                    panic!()
                                }
                            } else {
                                panic!()
                            }
                        } else {
                            panic!()
                        }
                    } else {
                        (false, syn::parse_quote!(true), pattern.clone(), pattern)
                    };

                let print1 =
                    gen_snprintf(to.clone(), limit.clone(), pattern1, args.clone(), is_file);
                let print2 = gen_snprintf(to, limit, pattern2, args, is_file);

                if has_condition {
                    *expr = syn::parse_quote!({
                        if #condition {
                            #print1
                        } else {
                            #print2
                        }
                    });
                } else {
                    *expr = print1;
                }
            }
        }

        syn::visit_mut::visit_expr_mut(self, expr);
    }
}

fn gen_snprintf(
    to: syn::Expr,
    limit: syn::Expr,
    pattern: syn::Expr,
    args: Vec<&syn::Expr>,
    is_file: bool,
) -> syn::Expr {
    if let syn::Expr::Cast(syn::ExprCast { expr: cast_one, .. }) = pattern {
        if let syn::Expr::Cast(syn::ExprCast { expr: cast_two, .. }) = cast_one.as_ref() {
            if let syn::Expr::Lit(ExprLit {
                lit: syn::Lit::ByteStr(s),
                ..
            }) = cast_two.as_ref()
            {
                let contents = s
                    .value()
                    .iter()
                    .cloned()
                    .take_while(|c| *c != 0)
                    .collect::<Vec<_>>();
                let format_string = std::str::from_utf8(&contents).unwrap();
                let arg_types = format_string.split("%").skip(1).map(|s| {
                    let before_format = s.chars().take_while(|c| c.is_ascii_digit());
                    let format = s.chars().skip_while(|c| c.is_ascii_digit()).next().unwrap();
                    before_format
                        .chain(std::iter::once(format))
                        .collect::<String>()
                });

                let args_transformed: Vec<syn::Expr> = args
                    .iter()
                    .zip(arg_types)
                    .map(|(arg, arg_type)| match arg_type.as_str() {
                        "s" => syn::parse_quote!(std::ffi::CStr::from_ptr(#arg).to_string_lossy()),
                        "c" => syn::parse_quote!(#arg as u8 as char),
                        "d" | "u" => (*arg).clone(),
                        "p" => syn::parse_quote!(#arg as *const os::raw::c_int),
                        o => {
                            if o.ends_with("X") {
                                (*arg).clone()
                            } else {
                                panic!(
                                    "Unknown format string type {} in {}",
                                    arg_type, format_string
                                )
                            }
                        }
                    })
                    .collect::<Vec<_>>();

                let format_string_cleaned = format_string
                    .replace("{", "{{")
                    .replace("}", "}}")
                    .replace("%s", "{}")
                    .replace("%c", "{}")
                    .replace("%d", "{}")
                    .replace("%u", "{}")
                    .replace("%p", "{:p}")
                    .replace("%2X", "{:02X}");

                if is_file {
                    syn::parse_quote!(fwrite!(#to, #format_string_cleaned, #(#args_transformed),*)
                        .unwrap_or(usize::MAX)
                        as os::raw::c_int)
                } else {
                    syn::parse_quote!(
                        snwrite!(#to, #limit as usize, #format_string_cleaned, #(#args_transformed),*)
                            .unwrap_or(usize::MAX) as os::raw::c_int
                    )
                }
            } else {
                panic!("{:?}", cast_two.to_token_stream().to_string());
            }
        } else {
            panic!()
        }
    } else {
        panic!("{:?}", pattern.to_token_stream().to_string());
    }
}

struct AssertEliminator {}
impl syn::visit_mut::VisitMut for AssertEliminator {
    fn visit_expr_block_mut(&mut self, i: &mut syn::ExprBlock) {
        i.block.stmts.iter_mut().for_each(|stmt| match stmt {
            syn::Stmt::Semi(syn::Expr::Call(c), _) => {
                if c.func.to_token_stream().to_string() == "__assert_fail" {
                    *stmt = syn::parse_quote!(panic!(););
                }
            }
            _ => {}
        });

        syn::visit_mut::visit_expr_block_mut(self, i);
    }
}

struct StackElementReplacer {}
impl syn::visit_mut::VisitMut for StackElementReplacer {
    fn visit_item_mut(&mut self, i: &mut syn::Item) {
        if let syn::Item::Struct(syn::ItemStruct {
            ident,
            fields: syn::Fields::Named(fields),
            ..
        })
        | syn::Item::Union(syn::ItemUnion { ident, fields, .. }) = i
        {
            if ident
                .to_token_stream()
                .to_string()
                .starts_with("C2RustUnnamed")
            {
                let field_names = fields
                    .named
                    .iter()
                    .map(|f| f.ident.clone())
                    .collect::<Vec<_>>();
                let name = ident.clone();

                if field_names
                    == vec![
                        Some(syn::parse_quote!(contents)),
                        Some(syn::parse_quote!(size)),
                        Some(syn::parse_quote!(capacity)),
                    ]
                {
                    let contents_type = fields
                        .named
                        .iter()
                        .find(|f| f.ident.clone() == syn::parse_quote!(contents))
                        .unwrap()
                        .ty
                        .clone();
                    *i = syn::parse_quote!(type #name = crate::core::util::StackElement<#contents_type>;);
                } else if field_names
                    == vec![
                        Some(syn::parse_quote!(long_data)),
                        Some(syn::parse_quote!(short_data)),
                    ]
                {
                    *i = syn::parse_quote!(type #name = crate::core::util::LongShortData;);
                } else if field_names
                    == vec![
                        Some(syn::parse_quote!(c2rust_unnamed)),
                        Some(syn::parse_quote!(external_scanner_state)),
                        Some(syn::parse_quote!(lookahead_char)),
                    ]
                {
                    *i = syn::parse_quote!(type #name = crate::core::util::ScannerStateWithLookahead;);
                } else if field_names
                    == vec![
                        Some(syn::parse_quote!(visible_child_count)),
                        Some(syn::parse_quote!(named_child_count)),
                        Some(syn::parse_quote!(visible_descendant_count)),
                        Some(syn::parse_quote!(dynamic_precedence)),
                        Some(syn::parse_quote!(repeat_depth)),
                        Some(syn::parse_quote!(production_id)),
                        Some(syn::parse_quote!(first_leaf)),
                    ]
                {
                    *i = syn::parse_quote!(type #name = crate::core::util::ScannerStateLookaheadMeta;);
                } else if field_names
                    == vec![
                        Some(syn::parse_quote!(symbol)),
                        Some(syn::parse_quote!(parse_state)),
                    ]
                {
                    *i = syn::parse_quote!(type #name = crate::core::util::ScannerStateLookaheadFirstLeaf;);
                } else if field_names
                    == vec![
                        Some(syn::parse_quote!(count)),
                        Some(syn::parse_quote!(reusable)),
                    ]
                {
                    *i = syn::parse_quote!(type #name = crate::core::util::ParseActionEntry;);
                } else if field_names
                    == vec![
                        Some(syn::parse_quote!(type_)),
                        Some(syn::parse_quote!(child_count)),
                        Some(syn::parse_quote!(symbol)),
                        Some(syn::parse_quote!(dynamic_precedence)),
                        Some(syn::parse_quote!(production_id)),
                    ]
                {
                    *i = syn::parse_quote!(type #name = crate::core::util::ParseActionReduce;);
                } else if field_names
                    == vec![
                        Some(syn::parse_quote!(type_)),
                        Some(syn::parse_quote!(state)),
                        Some(syn::parse_quote!(extra)),
                        Some(syn::parse_quote!(repetition)),
                    ]
                {
                    *i = syn::parse_quote!(type #name = crate::core::util::ParseActionShift;);
                } else if field_names
                    == vec![
                        Some(syn::parse_quote!(states)),
                        Some(syn::parse_quote!(symbol_map)),
                        Some(syn::parse_quote!(create)),
                        Some(syn::parse_quote!(destroy)),
                        Some(syn::parse_quote!(scan)),
                        Some(syn::parse_quote!(serialize)),
                        Some(syn::parse_quote!(deserialize)),
                    ]
                {
                    *i = syn::parse_quote!(type #name = crate::core::util::ExternalScanner;);
                }
            }
        }

        syn::visit_mut::visit_item_mut(self, i);
    }
}

struct StderrReplacer {}
impl syn::visit_mut::VisitMut for StderrReplacer {
    fn visit_expr_mut(&mut self, i: &mut syn::Expr) {
        if let syn::Expr::Path(syn::ExprPath { path, .. }) = i {
            if path.to_token_stream().to_string() == "stderr" {
                *i = syn::parse_quote!(core::ptr::null_mut());
            }
        }

        syn::visit_mut::visit_expr_mut(self, i);
    }
}

struct IsAsciiReplacer {}
impl syn::visit_mut::VisitMut for IsAsciiReplacer {
    fn visit_expr_mut(&mut self, i: &mut syn::Expr) {
        let to_match: syn::Expr = syn::parse_quote!(
            *(*__ctype_b_loc()).offset(chr as isize) as libc::c_int
                & _ISprint as libc::c_int as libc::c_ushort as libc::c_int
                != 0
        );

        if i.to_token_stream().to_string() == to_match.to_token_stream().to_string() {
            *i = syn::parse_quote!(((chr as u8).is_ascii_graphic() || chr == ' ' as i32));
        }

        syn::visit_mut::visit_expr_mut(self, i);
    }
}

struct ClockAfterDurationReplacer {}
impl syn::visit_mut::VisitMut for ClockAfterDurationReplacer {
    fn visit_expr_mut(&mut self, i: &mut syn::Expr) {
        if let syn::Expr::Path(p) = i {
            if p.path.is_ident("duration") {
                *i = syn::parse_quote!((duration as u64));
                return;
            }
        }

        syn::visit_mut::visit_expr_mut(self, i);
    }
}

fn main() {
    let mut public_apis = HashMap::new();
    public_apis.insert(
        "lib/src/api_raw.rs",
        vec![
            "TSInput",
            "TSInputEdit",
            "TSPoint",
            "TSRange",
            "TSTreeCursor",
            "TSLogger",
            "Length",
            "TSInputEncoding",
            "TSSymbolType",
            "TSLogType",
            "TSQuantifier",
            "TSQueryPredicateStepType",
            "TSQueryError",
            "TSInputEncodingUTF8",
            "TSInputEncodingUTF16",
            "TSSymbolTypeRegular",
            "TSSymbolTypeAnonymous",
            "TSSymbolTypeAuxiliary",
            "TSLogTypeParse",
            "TSLogTypeLex",
            "TSQuantifierZero",
            "TSQuantifierZeroOrOne",
            "TSQuantifierZeroOrMore",
            "TSQuantifierOne",
            "TSQuantifierOneOrMore",
            "TSQueryPredicateStepTypeDone",
            "TSQueryPredicateStepTypeCapture",
            "TSQueryPredicateStepTypeString",
            "TSQueryErrorNone",
            "TSQueryErrorSyntax",
            "TSQueryErrorNodeType",
            "TSQueryErrorField",
            "TSQueryErrorCapture",
            "TSQueryErrorStructure",
            "TSQueryErrorLanguage",
        ],
    );
    public_apis.insert(
        "lib/src/parser.rs",
        vec![
            "TSFieldMapEntry",
            "TSFieldMapSlice",
            "TSSymbolMetadata",
            "TSLexer",
            "TSParseAction",
            "TSParseActionType",
            "TSParseActionEntry",
            "TSLexMode",
            "TSClock",
            "TSParser",
            "timespec",
        ],
    );
    public_apis.insert("lib/src/lexer.rs", vec!["Lexer"]);
    public_apis.insert(
        "lib/src/subtree.rs",
        vec![
            "Subtree",
            "SubtreeHeapData",
            "SubtreeInlineData",
            "SubtreeArray",
            "SubtreePool",
            "MutableSubtree",
            "MutableSubtreeArray",
            "ExternalScannerState",
        ],
    );
    public_apis.insert("lib/src/node.rs", vec!["TSNode"]);
    public_apis.insert(
        "lib/src/tree_cursor.rs",
        vec!["TreeCursor", "TreeCursorEntry"],
    );
    public_apis.insert("lib/src/get_changed_ranges.rs", vec!["TSRangeArray"]);
    public_apis.insert("lib/src/language.rs", vec!["TSLanguage", "TableEntry"]);
    public_apis.insert("lib/src/tree.rs", vec!["TSTree"]);
    public_apis.insert(
        "lib/src/query.rs",
        vec![
            "TSQuery",
            "TSQueryCursor",
            "TSQueryMatch",
            "TSQueryPredicateStep",
        ],
    );
    public_apis.insert(
        "lib/src/stack.rs",
        vec![
            "StackSlice",
            "StackSliceArray",
            "StackSummary",
            "StackSummaryEntry",
            "Stack",
        ],
    );
    public_apis.insert("lib/src/wasm_store.rs", vec!["TSWasmStore"]);
    public_apis.insert("lib/src/imported_parser.rs", vec![]);

    public_apis
        .keys()
        .par_bridge()
        .progress_count(public_apis.len() as u64)
        .for_each(|file| {
            let loaded_file = std::fs::read_to_string(file).unwrap();
            let mut parsed_file = syn::parse_file(&loaded_file).unwrap();

            let apis_claimed_by_other = public_apis
                .iter()
                .filter(|(other_file, _)| *other_file != file)
                .flat_map(|(_, apis)| apis.iter())
                .collect::<Vec<_>>();

            let hidden_apis = vec![
                "uint64_t",
                "size_t",
                "ts_current_realloc",
                "ts_current_malloc",
                "ts_current_calloc",
                "ts_current_free",
                "atomic_inc",
                "atomic_dec",
                "atomic_load",
                "clock_now",
                "_ts_dup",
                "ts_tree_print_dot_graph",
            ];

            let fd_functions = vec!["ts_parser_print_dot_graphs", "ts_tree_print_dot_graph"];

            parsed_file.items.retain_mut(|item| {
                if let syn::Item::Fn(syn::ItemFn {
                    sig: syn::Signature { ident: i, .. },
                    block,
                    ..
                }) = item
                {
                    if i == "clock_after" {
                        (ClockAfterDurationReplacer {}).visit_block_mut(block);
                    }
                }

                match item.clone() {
                    syn::Item::ForeignMod(_) => false,
                    syn::Item::Struct(syn::ItemStruct { ident: i, .. })
                    | syn::Item::Union(syn::ItemUnion { ident: i, .. })
                    | syn::Item::Fn(syn::ItemFn {
                        sig: syn::Signature { ident: i, .. },
                        ..
                    })
                    | syn::Item::Static(syn::ItemStatic { ident: i, .. })
                    | syn::Item::Const(syn::ItemConst { ident: i, .. })
                    | syn::Item::Type(syn::ItemType { ident: i, .. }) => {
                        let is_fn = matches!(item, syn::Item::Fn(_));
                        if apis_claimed_by_other.contains(&&i.to_string().as_str())
                            || hidden_apis.contains(&&i.to_string().as_str())
                            || (*file == "lib/src/api_raw.rs" && is_fn)
                        {
                            false
                        } else if is_fn && fd_functions.contains(&&i.to_string().as_str()) {
                            // if let syn::Item::Fn(syn::ItemFn { block, .. }) = item {
                            //     *block = syn::parse_quote!({
                            //         if fd >= 0 {
                            //             panic!(
                            //                 "This function is not available in the c2rust
                            // conversion"             );
                            //         }
                            //     });
                            // } else {
                            //     panic!();
                            // }

                            true
                        } else {
                            if i.to_string() == "_IO_FILE" {
                                false
                            } else {
                                if i.to_string() == "FILE" {
                                    *item = syn::parse_quote!(
                                        use crate::core::util::libc::{
                                            FILE, fdopen, dup, fclose, fputs, fputc,
                                        };
                                    )
                                }

                                true
                            }
                        }
                    }
                    _ => true,
                }
            });

            parsed_file.items.insert(
                0,
                syn::parse_quote!(
                    use crate::core::util::*;
                ),
            );

            parsed_file.items.insert(
                0,
                syn::parse_quote!(
                    use crate::core::*;
                ),
            );

            let consts_to_add = parsed_file
                .items
                .iter()
                .filter_map(|item| {
                    if let syn::Item::Const(syn::ItemConst { ident: i, ty, .. }) = item {
                        if !ty.to_token_stream().to_string().starts_with("C2Rust") {
                            let new_name = syn::Ident::new(
                                &format!(
                                    "{}_{}",
                                    ty.to_token_stream().to_string(),
                                    i.to_token_stream().to_string()
                                ),
                                i.span(),
                            );
                            Some(syn::parse_quote!(pub const #new_name: #ty = #i;))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            parsed_file.items.extend(consts_to_add);

            (AllocReplacer {}).visit_file_mut(&mut parsed_file);
            (MemcpyReplacer {}).visit_file_mut(&mut parsed_file);
            (MemcmpReplacer {}).visit_file_mut(&mut parsed_file);
            (SnprintfReplacer {}).visit_file_mut(&mut parsed_file);
            (AssertEliminator {}).visit_file_mut(&mut parsed_file);
            (StackElementReplacer {}).visit_file_mut(&mut parsed_file);
            (StderrReplacer {}).visit_file_mut(&mut parsed_file);
            (IsAsciiReplacer {}).visit_file_mut(&mut parsed_file);

            std::fs::write(file, parsed_file.to_token_stream().to_string()).unwrap();
        });
}
