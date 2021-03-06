use syntax::ext::base::{Annotatable, ExtCtxt};
use syntax::codemap::Span;
use syntax::ast::{MetaItem, ItemKind, Block, VariantData, Ident, Ty};
use syntax::ptr::P;
use syntax::ext::build::AstBuilder;
use syntax::parse::token::InternedString;

use overrides;
use accepts;
use enums;

pub fn expand(ctx: &mut ExtCtxt,
              span: Span,
              _: &MetaItem,
              annotatable: &Annotatable,
              push: &mut FnMut(Annotatable)) {
    let item = match *annotatable {
        Annotatable::Item(ref item) => item,
        _ => {
            ctx.span_err(span, "#[derive(ToSql)] can only be applied to structs, single field \
                                tuple structs, and enums");
            return;
        }
    };

    let overrides = overrides::get_overrides(ctx, &item.attrs);
    let name = overrides.name.unwrap_or_else(|| item.ident.name.as_str());

    let (accepts_body, to_sql_body) = match item.node {
        ItemKind::Enum(ref def, _) => {
            let variants = enums::get_variants(ctx, "ToSql", def);
            (accepts::enum_body(ctx, name, &variants),
             enum_to_sql_body(ctx, span, item.ident, &variants))
        }
        ItemKind::Struct(VariantData::Tuple(ref fields, _), _) => {
            if fields.len() != 1 {
                ctx.span_err(span, "#[derive(ToSql)] can only be applied to structs, single field \
                                    tuple structs, and enums");
                return;
            }
            let inner = &fields[0].ty;

            (domain_accepts_body(ctx, name, inner), domain_to_sql_body(ctx))
        }
        ItemKind::Struct(VariantData::Struct(ref fields, _), _) => {
            let fields = fields.iter()
                               .map(|field| {
                                   let ident = field.ident.unwrap();
                                   let overrides = overrides::get_overrides(ctx, &field.attrs);
                                   let name = overrides.name.unwrap_or_else(|| ident.name.as_str());
                                   (name, ident, &*field.ty)
                               })
                               .collect::<Vec<_>>();
            let trait_ = quote_path!(ctx, ::postgres::types::ToSql);
            (accepts::composite_body(ctx, name, &fields, &trait_),
             composite_to_sql_body(ctx, span, &*fields))
        }
        _ => {
            ctx.span_err(span, "#[derive(ToSql)] can only be applied to structs, single field \
                                tuple structs, and enums");
            return;
        }
    };

    let type_ = item.ident;

    let item = quote_item!(ctx,
        impl ::postgres::types::ToSql for $type_ {
            to_sql_checked!();

            fn accepts(type_: &::postgres::types::Type) -> bool {
                $accepts_body
            }

            fn to_sql<W: ?::std::marker::Sized>(&self,
                                                _type: &::postgres::types::Type,
                                                out: &mut W,
                                                _info: &::postgres::types::SessionInfo)
                                                -> ::postgres::Result<::postgres::types::IsNull>
                where W: ::std::io::Write
            {
                $to_sql_body
            }
        }
    );

    push(Annotatable::Item(item.unwrap()));
}

fn domain_accepts_body(ctx: &mut ExtCtxt, name: InternedString, inner: &Ty) -> P<Block> {
    quote_block!(ctx, {
        match *type_.kind() {
            ::postgres::types::Kind::Domain(ref t) => {
                type_.name() == $name && <$inner as ::postgres::types::ToSql>::accepts(t)
            }
            _ => false
        }
    })
}

fn enum_to_sql_body(ctx: &mut ExtCtxt,
                    span: Span,
                    type_name: Ident,
                    variants: &[(Ident, InternedString)])
                    -> P<Block> {
    let mut arms = vec![];

    for &(ref variant_name, ref name) in variants {
        arms.push(quote_arm!(ctx, $type_name :: $variant_name => $name,));
    }

    let match_arg = ctx.expr_deref(span, ctx.expr_self(span));
    let match_ = ctx.expr_match(span, match_arg, arms);

    quote_block!(ctx, {
        let s = $match_;
        try!(::std::io::Write::write_all(out, s.as_bytes()));
        ::std::result::Result::Ok(::postgres::types::IsNull::No)
    })
}

fn domain_to_sql_body(ctx: &mut ExtCtxt) -> P<Block> {
    quote_block!(ctx, {
        let inner = match _type.kind() {
            &::postgres::types::Kind::Domain(ref inner) => inner,
            _ => unreachable!(),
        };
        ::postgres::types::ToSql::to_sql(&self.0, inner, out, _info)
    })
}

fn composite_to_sql_body(ctx: &mut ExtCtxt,
                         span: Span,
                         fields: &[(InternedString, Ident, &Ty)])
                         -> P<Block> {
    let mut arms = fields.iter()
                         .map(|&(ref name, ref ident, _)| {
                             quote_arm!(ctx, $name => {
                                 ::postgres::types::ToSql::to_sql(&self.$ident,
                                                                  field.type_(),
                                                                  &mut buf,
                                                                  _info)
                             })
                         })
                         .collect::<Vec<_>>();
    arms.push(quote_arm!(ctx, _ => unreachable!(),));
    let match_ = ctx.expr_match(span, quote_expr!(ctx, field.name()), arms);

    quote_block!(ctx, {
        let write_be_i32 = |w: &mut W, num: i32| {
            let buf = [(num >> 24) as u8, (num >> 16) as u8, (num >> 8) as u8, num as u8];
            ::std::io::Write::write_all(w, &buf)
        };

        let fields = match _type.kind() {
            &::postgres::types::Kind::Composite(ref fields) => fields,
            _ => unreachable!(),
        };

        try!(write_be_i32(out, fields.len() as i32));

        let mut buf = vec![];
        for field in fields {
            try!(write_be_i32(out, field.type_().oid() as i32));
            let r = $match_;
            match try!(r) {
                ::postgres::types::IsNull::Yes => try!(write_be_i32(out, -1)),
                ::postgres::types::IsNull::No => {
                    let len = if buf.len() > i32::max_value() as usize {
                        return ::std::result::Result::Err(::postgres::error::Error::Conversion(
                                "value too large to transmit".into()));
                    } else {
                        buf.len() as i32
                    };
                    try!(write_be_i32(out, len));
                    try!(::std::io::Write::write_all(out, &buf));
                }
            }
            buf.clear();
        }

        ::std::result::Result::Ok(::postgres::types::IsNull::No)
    })
}
