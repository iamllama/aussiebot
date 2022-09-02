use proc_macro::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::{
    parse::Parser, parse_macro_input, punctuated::Punctuated, spanned::Spanned, token::Comma,
    visit_mut::VisitMut, Attribute, DeriveInput, Expr, ExprRange, Field, Fields, Ident, ItemStruct,
    Lit, LitStr, Meta, NestedMeta, RangeLimits, Token,
};

#[derive(Debug, Clone)]
enum CmdType {
    Command,
    Filter,
    Timer,
}

impl Default for CmdType {
    fn default() -> Self {
        Self::Command
    }
}

impl From<CmdType> for proc_macro2::TokenStream {
    fn from(constr: CmdType) -> Self {
        match constr {
            CmdType::Command => quote! { crate::cmds::CmdType::Command },
            CmdType::Filter => quote! { crate::cmds::CmdType::Filter },
            CmdType::Timer => quote! { crate::cmds::CmdType::Timer },
        }
    }
}

#[derive(Debug, Clone)]
enum Constraint {
    None,
    NonEmpty,
    Positive,
    Negative,
    Range(ExprRange),
}

impl Default for Constraint {
    fn default() -> Self {
        Self::None
    }
}

impl From<Constraint> for proc_macro2::TokenStream {
    fn from(constr: Constraint) -> Self {
        match constr {
            Constraint::None => quote! { crate::cmds::Constraint::None },
            Constraint::NonEmpty => quote! { crate::cmds::Constraint::NonEmpty },
            Constraint::Positive => quote! { crate::cmds::Constraint::Positive },
            Constraint::Negative => quote! { crate::cmds::Constraint::Negative },
            Constraint::Range(r) => match r.limits {
                RangeLimits::Closed(_) => {
                    quote! { crate::cmds::Constraint::RangeClosed(#r) }
                }
                RangeLimits::HalfOpen(_) => {
                    quote! { crate::cmds::Constraint::RangeHalfOpen(#r) }
                }
            },
        }
    }
}

fn cmd_of(f: &Field) -> syn::Result<Option<&Attribute>> {
    let mut cmd_attrs = f.attrs.iter().filter(|a| a.path.is_ident("cmd"));
    let first = cmd_attrs.next();
    if let Some(attr) = cmd_attrs.next() {
        return Err(syn::Error::new(
            attr.span(),
            "only one `cmd` attribute allowed",
        ));
    }
    Ok(first)
}

#[derive(Default, Debug)]
struct CommandAttr {
    cmd_type: CmdType,
    locks: Vec<Ident>,
}

#[derive(Default, Debug)]
struct CmdFieldAttr {
    skip: bool,
    def_value: Option<Lit>,
    def_expr: Option<LitStr>,
    constr: Option<Constraint>,
}

fn err(err_cond: bool, spanned: &dyn Spanned, msg: impl Into<String>) -> syn::Result<()> {
    if err_cond {
        Err(syn::Error::new(spanned.span(), msg.into()))
    } else {
        Ok(())
    }
}

fn parse_cmd_field(f: &Field) -> syn::Result<Option<CmdFieldAttr>> {
    let cmd_attr = match cmd_of(f)? {
        Some(c) => c,
        None => return Ok(None),
    };

    let attr = cmd_attr.parse_meta()?;

    let meta_list = match attr {
        Meta::List(ml) => ml.nested,
        _ => return Err(syn::Error::new(attr.span(), "parsing error")),
    };

    let mut def_value: Option<Lit> = None;
    let mut def_expr: Option<LitStr> = None;
    let mut constr: Option<Constraint> = None;

    for sub_attr in meta_list.iter() {
        let sub_meta = match sub_attr {
            NestedMeta::Meta(m) => m,
            _ => return Err(syn::Error::new(sub_attr.span(), "unexpected literal")),
        };

        match sub_meta {
            Meta::Path(path) => {
                if path.is_ident("skip") {
                    return Ok(Some(CmdFieldAttr {
                        skip: true,
                        ..Default::default()
                    }));
                } else {
                    return Err(syn::Error::new(path.span(), "invalid attribute"));
                }
            }
            Meta::List(list) => {
                err(
                    list.nested.is_empty(),
                    sub_meta,
                    "unexpected empty attribute",
                )?;
                let value = list.nested.first().unwrap();
                if list.path.is_ident("def") {
                    err(
                        def_value.is_some() || def_expr.is_some(),
                        list,
                        "expected only one `def` or `defl` attribute",
                    )?;

                    let lit = match value {
                        NestedMeta::Lit(lit) => lit,
                        t => {
                            return Err(syn::Error::new(
                                value.span(),
                                format!("expected literal: {:#?}", t),
                            ))
                        }
                    };

                    def_value = Some(lit.clone());
                } else if list.path.is_ident("defl") {
                    err(
                        def_value.is_some() || def_expr.is_some(),
                        list,
                        "expected only one `def` or `defl` attribute",
                    )?;

                    let lit = match value {
                        NestedMeta::Lit(lit) => lit,
                        _ => {
                            return Err(syn::Error::new(
                                value.span(),
                                "expected string for `defl` attr",
                            ))
                        }
                    };

                    let ls = match lit {
                        Lit::Str(ls) => ls,
                        _ => {
                            return Err(syn::Error::new(
                                lit.span(),
                                "expected string for `defl` attr",
                            ))
                        }
                    };
                    def_expr = Some(ls.clone());
                } else if list.path.is_ident("constr") {
                    err(
                        list.nested.len() != 1,
                        list,
                        "at most one constraint allowed per field",
                    )?;

                    err(
                        constr.is_some(),
                        list,
                        "at most one constraint allowed per field",
                    )?;

                    match value {
                        NestedMeta::Lit(_) => {
                            return Err(syn::Error::new(
                                value.span(),
                                "expected constraint, got literal",
                            ));
                        }
                        NestedMeta::Meta(Meta::NameValue(ref nv)) => {
                            let range_lit = match &*nv.path.get_ident().unwrap().to_string() {
                                "range" => &nv.lit,
                                _ => return Err(syn::Error::new(nv.span(), "expected `range`")),
                            };

                            let range_lit = match range_lit {
                                Lit::Str(ls) => ls,
                                _ => {
                                    return Err(syn::Error::new(
                                        nv.span(),
                                        "expected string in `range`",
                                    ))
                                }
                            };

                            let range =
                                syn::parse_str::<ExprRange>(&range_lit.value()).map_err(|e| {
                                    syn::Error::new(
                                        range_lit.span(),
                                        format!("invalid range: {}", e),
                                    )
                                })?;

                            err(
                                range.from.is_none() || range.to.is_none(),
                                range_lit,
                                "both ends of the range must be specified",
                            )?;

                            //eprintln!("constr range: {:#?}", range);
                            constr = Some(Constraint::Range(range));
                        }
                        NestedMeta::Meta(Meta::Path(path)) => {
                            //eprintln!("constr path: {:?}", path.get_ident());
                            let constraint = match &*path.get_ident().unwrap().to_string() {
                                "pos" => Constraint::Positive,
                                "neg" => Constraint::Negative,
                                "non_empty" => Constraint::NonEmpty,
                                _ => {
                                    return Err(syn::Error::new(
                                        path.span(),
                                        "expected `pos`, `neg` or `non_empty`",
                                    ))
                                }
                            };

                            constr = Some(constraint);
                        }
                        _ => {
                            return Err(syn::Error::new(value.span(), "invalid `constr` attribute"))
                        }
                    }
                } else {
                    err(true, sub_meta, "unknown attribute")?
                };
            }
            _ => return Err(syn::Error::new(sub_attr.span(), "unexpected named value")),
        }
    }

    Ok(Some(CmdFieldAttr {
        skip: false,
        def_value,
        def_expr,
        constr,
    }))
}

fn parse_cmd_struct(meta_list: &Punctuated<NestedMeta, Comma>) -> syn::Result<Option<CommandAttr>> {
    let mut cmd_type: Option<CmdType> = None;
    let mut locks: Option<Vec<Ident>> = None;

    for sub_attr in meta_list.iter() {
        let sub_meta = match sub_attr {
            NestedMeta::Meta(m) => m,
            _ => return Err(syn::Error::new(sub_attr.span(), "unexpected literal")),
        };

        match sub_meta {
            Meta::Path(path) => {
                if cmd_type.is_some() {
                    return Err(syn::Error::new(
                        sub_attr.span(),
                        "command type already declared",
                    ));
                }
                if path.is_ident("cmd") {
                    cmd_type = Some(CmdType::Command);
                } else if path.is_ident("filter") {
                    cmd_type = Some(CmdType::Filter);
                } else if path.is_ident("timer") {
                    cmd_type = Some(CmdType::Timer);
                } else {
                    return Err(syn::Error::new(path.span(), "invalid attribute"));
                }
            }
            Meta::List(list) => {
                err(
                    list.nested.is_empty(),
                    sub_meta,
                    "unexpected empty attribute",
                )?;
                if list.path.is_ident("locks") {
                    if locks.is_some() {
                        return Err(syn::Error::new(sub_attr.span(), "locks already declared"));
                    }
                    let mut _locks = vec![];
                    for lock in list.nested.iter() {
                        let lock = match lock {
                            NestedMeta::Meta(Meta::Path(path)) => path.get_ident().unwrap().clone(),
                            _ => return Err(syn::Error::new(lock.span(), "invalid lock")),
                        };
                        if _locks.contains(&lock) {
                            return Err(syn::Error::new(
                                lock.span(),
                                "lock of the same name already declared",
                            ));
                        }
                        _locks.push(lock);
                    }
                    locks = Some(_locks);
                } else {
                    err(true, sub_meta, "unknown attribute")?
                };
            }
            _ => return Err(syn::Error::new(sub_attr.span(), "unexpected named value")),
        }
    }

    Ok(Some(CommandAttr {
        cmd_type: cmd_type.unwrap_or_default(),
        locks: locks.unwrap_or_default(),
    }))
}

// TODO: only yse first doc string as description
fn doc<'a>(attrs: impl Iterator<Item = &'a Attribute>) -> String {
    let docstrings: Vec<String> = attrs
        .filter_map(|attr| {
            let meta = match attr.parse_meta() {
                Ok(Meta::NameValue(nv)) if nv.path.is_ident("doc") => nv,
                _ => return None,
            };
            match meta.lit {
                Lit::Str(s) => Some(s.value().trim().to_owned()),
                _ => None,
            }
        })
        .collect();

    docstrings.join("\n")
}

fn emit_fn_new<'a>(
    fields: impl Iterator<Item = &'a Field>,
    name: &'a Ident,
    cmd_attrs: &'a [CmdFieldAttr],
    autocorrect: bool,
) -> proc_macro2::TokenStream {
    let new_fields = fields.zip(cmd_attrs).map(|(field, cmd)| {
        if cmd.skip {
            return quote! {};
        }

        let fname = field.ident.as_ref().unwrap();
        let fty = &field.ty;

        let constr = cmd.constr.clone().unwrap_or_default();
        let constr: proc_macro2::TokenStream = constr.into();

        quote! {
          if let Some(value) = kv.remove(stringify!(#fname)) {
            if !value.verify(#constr) {
              println!(concat!("failed verification: ", stringify!(#fname)));
              return None;
            }

            let value = <#fty>::try_from(value);
            match value {
              Ok(value) => {
                cmd.#fname = value;
              },
              Err(e) => {
                ::tracing::warn!(key=stringify!(#fname), cmd=stringify!(#name), name=cmd.name.as_str(), "{}", e)
              }
            }
          }
        }
    });

    let autocorrect = if autocorrect {
        quote! {
          if !cmd.prefix.is_empty() {
            // build DFA
            cmd.levenshtein = Some(crate::cmds::DFAWrapper(crate::cmds::DFA_BUILDER.build_dfa(&cmd.prefix)));
          }
        }
    } else {
        quote! {}
    };

    quote! {
        fn new(name: impl Into<String>, kv: &mut [(String, crate::cmds::Value)]) -> Option<Self> {
          use crate::cmds::VerifyConstraint;

          let mut cmd = <#name>::default();
          cmd.name = name.into();
          let mut kv: ::std::collections::HashMap<String, crate::cmds::Value> = kv.iter_mut().map(std::mem::take).collect();
          #(#new_fields)*
          #autocorrect
          Some(cmd)
        }
    }
}

fn emit_fn_def<'a>(
    fields: impl Iterator<Item = &'a Field>,
    name: &'a Ident,
    cmd_attrs: &'a [CmdFieldAttr],
) -> proc_macro2::TokenStream {
    let (default_fields, asserts): (Vec<proc_macro2::TokenStream>, Vec<proc_macro2::TokenStream>) = fields.zip(cmd_attrs).map(|(field, cmd)| {
        let fname = match field.ident.as_ref() {
            Some(i) => i,
            None => return (syn::Error::new(field.span(), "missing field ident").to_compile_error(), quote! {}),
        };
        let fty = &field.ty;

        let field_ts = if let Some(ref def) = cmd.def_value {
            quote! {
              #fname: #def.into()
            }
        } else if let Some(ref expr) = cmd.def_expr {
            // https://github.com/dtolnay/syn/issues/868
            let expr = match syn::parse_str::<Expr>(&expr.value()) {
                Ok(t) => t,
                Err(e) => {
                    return (syn::Error::new(expr.span(), format!("invalid expr: {}", e))
                        .to_compile_error(), quote! {})
                }
            };
            quote! { #fname: #expr }
        } else {
            quote! {
              #fname: <#fty>::default()
            }
        };

        let fconstr: proc_macro2::TokenStream = cmd.constr.clone().unwrap_or_default().into();
        let assert_ts = if !cmd.skip {
            quote! {
              assert!(ret.#fname.verify(#fconstr), "default {}.{} failed constraint {:?}", stringify!(#name), stringify!(#fname), #fconstr);
            }
        } else {
            quote! {}
        };

        (field_ts, assert_ts)
    }).unzip();

    quote! {
        impl Default for #name {
          fn default() -> Self {
              use crate::cmds::VerifyConstraint;

              let ret = Self {
                #(#default_fields),*
              };
              #(#asserts)*
              ret
          }
      }
    }
}

fn emit_fns_schema_dump<'a>(
    fields: impl Iterator<Item = &'a Field>,
    name: &'a Ident,
    cmd_type: CmdType,
    cmd_attrs: &'a [CmdFieldAttr],
    cmd_doc: &str,
) -> proc_macro2::TokenStream {
    let cmd_doc = syn::Lit::new(proc_macro2::Literal::string(cmd_doc));

    let (field_schemas, field_dumps): (Vec<proc_macro2::TokenStream>, Vec<proc_macro2::TokenStream>) = fields.zip(cmd_attrs).flat_map(|(f, cmd)| {
        if cmd.skip {
          return None;
        }
        let fname = f.ident.as_ref().unwrap();
        //let fty = &f.ty;
        let doc_str = doc(f.attrs.iter());
        let mut fdesc = syn::Lit::new(proc_macro2::Literal::string(&*doc_str));
        fdesc.set_span(f.span());
        let constr: proc_macro2::TokenStream = cmd.constr.clone().unwrap_or_default().into();
        Some((
            quote! {
                (stringify!(#fname).to_owned(), #fdesc.to_owned(), crate::cmds::Value::from(cmd.#fname), #constr)
            },
            quote! {
              (stringify!(#fname).to_owned(), crate::cmds::Value::from(self.#fname.clone()))
            },
        ))
    }).unzip();

    let cmd_type: proc_macro2::TokenStream = cmd_type.into();

    quote! {
      fn schema(platform: crate::msg::Platform) -> crate::cmds::CmdSchema {
        use crate::cmds::CmdDesc;

        let cmd = #name::default();
        (stringify!(#name).to_owned(), cmd.description(platform).unwrap_or_else(|| #cmd_doc.to_owned()), #cmd_type, vec![#(#field_schemas),*])
      }

      fn dump(&self) -> crate::cmds::CmdDump {
        (stringify!(#name).to_owned(), self.name.clone(), vec![#(#field_dumps),*])
      }
    }
}

fn emit_locks(name: &'_ Ident, locks: Vec<Ident>) -> proc_macro2::TokenStream {
    let lock_keys = locks
        .iter()
        .map(|lock| {
            (
                lock,
                format_ident!(
                    "{}_LOCK_{}",
                    syn::parse_str::<syn::Ident>(&name.to_string().to_uppercase()).unwrap(),
                    syn::parse_str::<syn::Ident>(&lock.to_string().to_uppercase()).unwrap()
                ),
            )
        })
        .map(|(lock, lock_key)| quote_spanned! {lock.span()=> #lock_key});
    let lock_values = locks
        .iter()
        .map(|lock| {
            (
                lock,
                format_ident!(
                    "{}_{}",
                    syn::parse_str::<syn::Ident>(&name.to_string().to_lowercase()).unwrap(),
                    lock
                ),
            )
        })
        .map(|(lock, lock_val)| quote_spanned! {lock.span()=> #lock_val});

    quote! {
      #(
        pub(crate) static #lock_keys: ::once_cell::sync::Lazy<String> = ::once_cell::sync::Lazy::new(|| format!(concat!("aussiebot_{}_", stringify!(#lock_values)), &*crate::CHANNEL_NAME));
      )*
    }
}

fn emit_fn_args_schema<'a>(
    mut fields: impl Iterator<Item = &'a Field>,
    cmd_doc: &'a str,
) -> proc_macro2::TokenStream {
    let cmd_doc = syn::Lit::new(proc_macro2::Literal::string(cmd_doc));

    let has_prefix = fields.any(|field| field.ident.as_ref().unwrap() == "prefix");
    if !has_prefix {
        return quote! {};
    }

    quote! {
      fn args_schema(&self, platform: Platform) -> Option<crate::cmds::ArgDump> {
        use crate::cmds::Invokable;
        use crate::cmds::CmdDesc;

        if self.enabled && !self.prefix.is_empty() && self.platform().contains(platform) {
          let prefix = crate::cmds::unbang_prefix(&self.prefix);
          Some((
            prefix.to_owned(),
            self.description(platform).unwrap_or_else(|| #cmd_doc.to_owned()),
            self.hidden(platform),
            self.perms,
            self.args(platform),
          ))
        } else {
            None
        }
      }
    }
}

fn emit_command(
    args: &Punctuated<NestedMeta, Comma>,
    st: &ItemStruct,
    autocorrect: bool,
) -> proc_macro2::TokenStream {
    let name = &st.ident;
    let fields = match st.fields {
        Fields::Named(ref fields) => &fields.named,
        _ => return quote! { compile_error!("expected a struct with named fields"); },
    };
    let doc_string = doc(st.attrs.iter());

    let maybe_struct_cmd = match parse_cmd_struct(args) {
        Ok(x) => x,
        Err(e) => return e.to_compile_error(),
    };

    let cmd_type = maybe_struct_cmd
        .as_ref()
        .map(|top_attr| top_attr.cmd_type.clone())
        .unwrap_or_default();

    let mut cmd_attrs = vec![];
    for cmd_attr in fields.iter().map(parse_cmd_field) {
        let cmd = match cmd_attr {
            Ok(Some(d)) => d,
            Ok(None) => CmdFieldAttr::default(),
            Err(e) => return e.to_compile_error(),
        };
        cmd_attrs.push(cmd);
    }
    let cmd_attrs = cmd_attrs;

    let impl_def = emit_fn_def(fields.iter(), name, &cmd_attrs);
    let fn_new = emit_fn_new(fields.iter(), name, &cmd_attrs, autocorrect);
    let fns_schema_dump =
        emit_fns_schema_dump(fields.iter(), name, cmd_type, &cmd_attrs, &doc_string);
    let locks = maybe_struct_cmd
        .map(|top_attr| emit_locks(name, top_attr.locks))
        .unwrap_or_default();
    let fn_arg_schema = emit_fn_args_schema(fields.iter(), &doc_string);

    quote! {
      use crate::cmds::VerifyConstraint;
      #locks
      #impl_def
      impl crate::cmds::Commandable for #name {
        #fn_new
        #fns_schema_dump
        #fn_arg_schema
      }
    }
}

#[derive(Debug, Default)]
struct AddFields {
    prefix: bool,
    autocorrect: bool,
}

impl VisitMut for AddFields {
    // fn visit_item_struct_mut(&mut self, st: &mut ItemStruct) {
    //     st.attrs.first().map(|attr| attr);
    //     syn::visit_mut::visit_item_struct_mut(self, st)
    // }

    fn visit_fields_named_mut(&mut self, fields: &mut syn::FieldsNamed) {
        for field in &fields.named {
            if field.ident.as_ref().unwrap() == "autocorrect" {
                self.autocorrect = true;
            } else if field.ident.as_ref().unwrap() == "prefix" {
                self.prefix = true;
            }
        }

        let levenshtein = if self.autocorrect {
            quote! {
              /// optional DFA for prefix autocorrection
              #[cmd(skip)]
              levenshtein: Option<crate::cmds::DFAWrapper>,
            }
        } else {
            quote! {}
        };

        let old_f = &fields.named;
        let new_f: syn::FieldsNamed = syn::parse_quote! {
          {
            /// Command name
            #[cmd(skip)]
            name: String,
            #levenshtein
            /// Command enabled
            enabled: bool,
            #old_f
          }
        };
        *fields = new_f;

        syn::visit_mut::visit_fields_named_mut(self, fields);
    }
}

#[derive(Debug, Default)]
struct SanitiseFields {}

impl VisitMut for SanitiseFields {
    fn visit_item_struct_mut(&mut self, st: &mut ItemStruct) {
        st.attrs.retain(|attr| !attr.path.is_ident("cmd"));
        syn::visit_mut::visit_item_struct_mut(self, st)
    }

    fn visit_fields_named_mut(&mut self, fields: &mut syn::FieldsNamed) {
        fields.named.iter_mut().for_each(|field| {
            field.attrs.retain(|attr| !attr.path.is_ident("cmd"));
            field.vis = syn::parse_quote! { pub(crate) };
        });
        syn::visit_mut::visit_fields_named_mut(self, fields);
    }
}

#[proc_macro_attribute]
pub fn command(args: TokenStream, input: TokenStream) -> TokenStream {
    let parser = Punctuated::<NestedMeta, Token![,]>::parse_separated_nonempty;
    let args: Punctuated<NestedMeta, Token![,]> = match parser.parse(args) {
        Ok(m) => m,
        Err(e) => return e.to_compile_error().into(),
    };
    let mut st = parse_macro_input!(input as syn::ItemStruct);

    // add fields
    let mut cmd = AddFields::default();
    cmd.visit_item_struct_mut(&mut st);

    // codegen
    let emitted = emit_command(&args, &st, cmd.prefix && cmd.autocorrect);

    // strip cmd attrs
    let mut st = st;
    SanitiseFields::default().visit_item_struct_mut(&mut st);

    quote! {
      #[derive(Debug)]
      #st
      #emitted
    }
    .into()
}

#[proc_macro_derive(Invokable)]
pub fn invoke(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let cmd_name = ast.ident;
    quote! {
      impl Invokable for #cmd_name {}
    }
    .into()
}
