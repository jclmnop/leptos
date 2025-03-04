use proc_macro2::{Ident, Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::{spanned::Spanned, Expr, ExprLit, ExprPath, Lit};
use syn_rsx::{Node, NodeAttribute, NodeElement, NodeName};

use crate::{is_component_node, Mode};

#[derive(Clone, Copy)]
enum TagType {
    Unknown,
    Html,
    Svg,
    Math,
}

const TYPED_EVENTS: [&str; 126] = [
    "afterprint",
    "beforeprint",
    "beforeunload",
    "gamepadconnected",
    "gamepaddisconnected",
    "hashchange",
    "languagechange",
    "message",
    "messageerror",
    "offline",
    "online",
    "pagehide",
    "pageshow",
    "popstate",
    "rejectionhandled",
    "storage",
    "unhandledrejection",
    "unload",
    "abort",
    "animationcancel",
    "animationend",
    "animationiteration",
    "animationstart",
    "auxclick",
    "beforeinput",
    "blur",
    "canplay",
    "canplaythrough",
    "change",
    "click",
    "close",
    "compositionend",
    "compositionstart",
    "compositionupdate",
    "contextmenu",
    "cuechange",
    "dblclick",
    "drag",
    "dragend",
    "dragenter",
    "dragleave",
    "dragover",
    "dragstart",
    "drop",
    "durationchange",
    "emptied",
    "ended",
    "error",
    "focus",
    "focusin",
    "focusout",
    "formdata",
    "gotpointercapture",
    "input",
    "invalid",
    "keydown",
    "keypress",
    "keyup",
    "load",
    "loadeddata",
    "loadedmetadata",
    "loadstart",
    "lostpointercapture",
    "mousedown",
    "mouseenter",
    "mouseleave",
    "mousemove",
    "mouseout",
    "mouseover",
    "mouseup",
    "pause",
    "play",
    "playing",
    "pointercancel",
    "pointerdown",
    "pointerenter",
    "pointerleave",
    "pointermove",
    "pointerout",
    "pointerover",
    "pointerup",
    "progress",
    "ratechange",
    "reset",
    "resize",
    "scroll",
    "securitypolicyviolation",
    "seeked",
    "seeking",
    "select",
    "selectionchange",
    "selectstart",
    "slotchange",
    "stalled",
    "submit",
    "suspend",
    "timeupdate",
    "toggle",
    "touchcancel",
    "touchend",
    "touchmove",
    "touchstart",
    "transitioncancel",
    "transitionend",
    "transitionrun",
    "transitionstart",
    "volumechange",
    "waiting",
    "webkitanimationend",
    "webkitanimationiteration",
    "webkitanimationstart",
    "webkittransitionend",
    "wheel",
    "DOMContentLoaded",
    "devicemotion",
    "deviceorientation",
    "orientationchange",
    "copy",
    "cut",
    "paste",
    "fullscreenchange",
    "fullscreenerror",
    "pointerlockchange",
    "pointerlockerror",
    "readystatechange",
    "visibilitychange",
];

pub(crate) fn render_view(cx: &Ident, nodes: &[Node], mode: Mode) -> TokenStream {
    if mode == Mode::Ssr {
        if nodes.is_empty() {
            let span = Span::call_site();
            quote_spanned! {
                span => leptos::Unit
            }
        } else if nodes.len() == 1 {
            root_node_to_tokens_ssr(cx, &nodes[0])
        } else {
            fragment_to_tokens_ssr(cx, Span::call_site(), nodes)
        }
    } else if nodes.is_empty() {
        let span = Span::call_site();
        quote_spanned! {
            span => leptos::Unit
        }
    } else if nodes.len() == 1 {
        node_to_tokens(cx, &nodes[0], TagType::Unknown)
    } else {
        fragment_to_tokens(cx, Span::call_site(), nodes, false, TagType::Unknown)
    }
}

fn root_node_to_tokens_ssr(cx: &Ident, node: &Node) -> TokenStream {
    match node {
        Node::Fragment(fragment) => {
            fragment_to_tokens_ssr(cx, Span::call_site(), &fragment.children)
        }
        Node::Comment(_) | Node::Doctype(_) | Node::Attribute(_) => quote! {},
        Node::Text(node) => {
            let value = node.value.as_ref();
            quote! {
                leptos::text(#value)
            }
        }
        Node::Block(node) => {
            let value = node.value.as_ref();
            quote! {
                #[allow(unused_braces)]
                #value
            }
        }
        Node::Element(node) => root_element_to_tokens_ssr(cx, node),
    }
}

fn fragment_to_tokens_ssr(cx: &Ident, _span: Span, nodes: &[Node]) -> TokenStream {
    let nodes = nodes.iter().map(|node| {
        let node = root_node_to_tokens_ssr(cx, node);
        quote! {
            #node.into_view(#cx)
        }
    });
    quote! {
        {
            leptos::Fragment::new(vec![
                #(#nodes),*
            ])
        }
    }
}

fn root_element_to_tokens_ssr(cx: &Ident, node: &NodeElement) -> TokenStream {
    if is_component_node(&node) {
        component_to_tokens(cx, node)
    } else {
        let mut template = String::new();
        let mut holes = Vec::<TokenStream>::new();
        let mut exprs_for_compiler = Vec::<TokenStream>::new();

        element_to_tokens_ssr(
            cx,
            node,
            &mut template,
            &mut holes,
            &mut exprs_for_compiler,
            true,
        );

        let template = if holes.is_empty() {
            quote! {
            #template
            }
        } else {
            quote! {
            format!(
                #template,
                #(#holes)*
            )
            }
        };

        let tag_name = node.name.to_string();
        let typed_element_name = Ident::new(&camel_case_tag_name(&tag_name), node.name.span());
        quote! {
        {
            #(#exprs_for_compiler)*
            ::leptos::HtmlElement::from_html(cx, leptos::#typed_element_name::default(), #template)
        }
        }
    }
}

fn element_to_tokens_ssr(
    cx: &Ident,
    node: &NodeElement,
    template: &mut String,
    holes: &mut Vec<TokenStream>,
    exprs_for_compiler: &mut Vec<TokenStream>,
    is_root: bool,
) {
    if is_component_node(node) {
        template.push_str("{}");
        let component = component_to_tokens(cx, node);
        holes.push(quote! {
          {#component}.into_view(cx).render_to_string(cx),
        })
    } else {
        template.push('<');
        template.push_str(&node.name.to_string());

        for attr in &node.attributes {
            if let Node::Attribute(attr) = attr {
                attribute_to_tokens_ssr(cx, attr, template, holes, exprs_for_compiler);
            }
        }

        // insert hydration ID
        let hydration_id = if is_root {
            quote! { leptos::HydrationCtx::peek(), }
        } else {
            quote! { leptos::HydrationCtx::id(), }
        };
        match node
            .attributes
            .iter()
            .find(|node| matches!(node, Node::Attribute(attr) if attr.key.to_string() == "id"))
        {
            Some(_) => {
                template.push_str(&format!(" leptos-hk=\"_{{}}\""));
            }
            None => {
                template.push_str(&format!(" id=\"_{{}}\""));
            }
        }
        holes.push(hydration_id);

        set_class_attribute_ssr(cx, node, template, holes);

        if is_self_closing(node) {
            template.push_str("/>");
        } else {
            template.push('>');
            for child in &node.children {
                match child {
                    Node::Element(child) => {
                        element_to_tokens_ssr(cx, child, template, holes, exprs_for_compiler, false)
                    }
                    Node::Text(text) => {
                        if let Some(value) = value_to_string(&text.value) {
                            template.push_str(&value);
                        } else {
                            template.push_str("{}");
                            let value = text.value.as_ref();

                            holes.push(quote! {
                              #value.into_view(#cx).render_to_string(#cx),
                            })
                        }
                    }
                    Node::Block(block) => {
                        if let Some(value) = value_to_string(&block.value) {
                            template.push_str(&value);
                        } else {
                            template.push_str("{}");
                            let value = block.value.as_ref();
                            holes.push(quote! {
                              #value.into_view(#cx).render_to_string(#cx),
                            })
                        }
                    }
                    Node::Fragment(_) => todo!(),
                    _ => {}
                }
            }

            template.push_str("</");
            template.push_str(&node.name.to_string());
            template.push('>');
        }
    }
}

fn value_to_string(value: &syn_rsx::NodeValueExpr) -> Option<String> {
    match &value.as_ref() {
        syn::Expr::Lit(lit) => match &lit.lit {
            syn::Lit::Str(s) => Some(s.value()),
            syn::Lit::Char(c) => Some(c.value().to_string()),
            syn::Lit::Int(i) => Some(i.base10_digits().to_string()),
            syn::Lit::Float(f) => Some(f.base10_digits().to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn attribute_to_tokens_ssr(
    cx: &Ident,
    node: &NodeAttribute,
    template: &mut String,
    holes: &mut Vec<TokenStream>,
    exprs_for_compiler: &mut Vec<TokenStream>,
) {
    let name = node.key.to_string();
    if name == "ref" || name == "_ref" {
        // ignore refs on SSR
    } else if let Some(name) = name.strip_prefix("on:") {
        let handler = node
            .value
            .as_ref()
            .expect("event listener attributes need a value")
            .as_ref();

        #[allow(unused_variables)]
        let (name, is_force_undelegated) = parse_event(name);

        let event_type = TYPED_EVENTS
            .iter()
            .find(|e| **e == name)
            .copied()
            .unwrap_or("Custom");
        let event_type = event_type
            .parse::<TokenStream>()
            .expect("couldn't parse event name");

        let event_type = if is_force_undelegated {
            quote! { ::leptos::ev::undelegated(::leptos::ev::#event_type) }
        } else {
            quote! { ::leptos::ev::#event_type }
        };
        exprs_for_compiler.push(quote! {
            leptos::ssr_event_listener(#event_type, #handler);
        })
    } else if name.strip_prefix("prop:").is_some() || name.strip_prefix("class:").is_some() {
        // ignore props for SSR
        // ignore classes: we'll handle these separately
    } else {
        let name = name.replacen("attr:", "", 1);

        if name != "class" {
            template.push(' ');
            template.push_str(&name);

            if let Some(value) = node.value.as_ref() {
                if let Some(value) = value_to_string(value) {
                    template.push_str("=\"");
                    template.push_str(&value);
                    template.push('"');
                } else {
                    template.push_str("=\"{}\"");
                    let value = value.as_ref();
                    holes.push(quote! {
                      leptos::escape_attr(&{#value}.into_attribute(#cx).as_nameless_value_string()),
                    })
                }
            }
        }
    }
}

fn set_class_attribute_ssr(
    cx: &Ident,
    node: &NodeElement,
    template: &mut String,
    holes: &mut Vec<TokenStream>,
) {
    let static_class_attr = node
        .attributes
        .iter()
        .filter_map(|a| {
            if let Node::Attribute(a) = a {
                if a.key.to_string() == "class" {
                    a.value.as_ref().and_then(value_to_string)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    let dyn_class_attr = node
        .attributes
        .iter()
        .filter_map(|a| {
            if let Node::Attribute(a) = a {
                if a.key.to_string() == "class" {
                    if a.value.as_ref().and_then(value_to_string).is_some()
                        || fancy_class_name(&a.key.to_string(), cx, a).is_some()
                    {
                        None
                    } else {
                        Some((a.key.span(), &a.value))
                    }
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let class_attrs = node
        .attributes
        .iter()
        .filter_map(|node| {
            if let Node::Attribute(node) = node {
                let name = node.key.to_string();
                if name == "class" {
                    return if let Some((_, name, value)) = fancy_class_name(&name, cx, node) {
                        let span = node.key.span();
                        Some((span, name, value))
                    } else {
                        None
                    };
                }
                if name.starts_with("class:") || name.starts_with("class-") {
                    let name = if name.starts_with("class:") {
                        name.replacen("class:", "", 1)
                    } else if name.starts_with("class-") {
                        name.replacen("class-", "", 1)
                    } else {
                        name
                    };
                    let value = node
                        .value
                        .as_ref()
                        .expect("class: attributes need values")
                        .as_ref();
                    let span = node.key.span();
                    Some((span, name, value))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if !static_class_attr.is_empty() || !dyn_class_attr.is_empty() || !class_attrs.is_empty() {
        template.push_str(" class=\"");

        template.push_str(&static_class_attr);

        for (_span, value) in dyn_class_attr {
            if let Some(value) = value {
                template.push_str(" {}");
                let value = value.as_ref();
                holes.push(quote! {
                  leptos::escape_attr(&(cx, #value).into_attribute(#cx).as_nameless_value_string()),
                });
            }
        }

        for (_span, name, value) in &class_attrs {
            template.push_str(" {}");
            holes.push(quote! {
              (cx, #value).into_class(#cx).as_value_string(#name),
            });
        }

        template.push('"');
    }
}

fn fragment_to_tokens(
    cx: &Ident,
    _span: Span,
    nodes: &[Node],
    lazy: bool,
    parent_type: TagType,
) -> TokenStream {
    let nodes = nodes.iter().map(|node| {
        let node = node_to_tokens(cx, node, parent_type);

        quote! {
            #node.into_view(#cx)
        }
    });
    if lazy {
        quote! {
            {
                leptos::Fragment::lazy(|| vec![
                    #(#nodes),*
                ])
            }
        }
    } else {
        quote! {
            {
                leptos::Fragment::new(vec![
                    #(#nodes),*
                ])
            }
        }
    }
}

fn node_to_tokens(cx: &Ident, node: &Node, parent_type: TagType) -> TokenStream {
    match node {
        Node::Fragment(fragment) => fragment_to_tokens(
            cx,
            Span::call_site(),
            &fragment.children,
            false,
            parent_type,
        ),
        Node::Comment(_) | Node::Doctype(_) => quote! {},
        Node::Text(node) => {
            let value = node.value.as_ref();
            quote! {
                leptos::text(#value)
            }
        }
        Node::Block(node) => {
            let value = node.value.as_ref();
            quote! { #value }
        }
        Node::Attribute(node) => attribute_to_tokens(cx, node),
        Node::Element(node) => element_to_tokens(cx, node, parent_type),
    }
}

fn element_to_tokens(cx: &Ident, node: &NodeElement, mut parent_type: TagType) -> TokenStream {
    if is_component_node(node) {
        component_to_tokens(cx, node)
    } else {
        let tag = node.name.to_string();
        let name = if is_custom_element(&tag) {
            let name = node.name.to_string();
            quote! { leptos::leptos_dom::custom(#cx, leptos::leptos_dom::Custom::new(#name)) }
        } else if is_svg_element(&tag) {
            let name = &node.name;
            parent_type = TagType::Svg;
            quote! { leptos::leptos_dom::svg::#name(#cx) }
        } else if is_math_ml_element(&tag) {
            let name = &node.name;
            parent_type = TagType::Math;
            quote! { leptos::leptos_dom::math::#name(#cx) }
        } else if is_ambiguous_element(&tag) {
            let name = &node.name;
            match parent_type {
                TagType::Unknown => {
                    // We decided this warning was too aggressive, but I'll leave it here in case we want it later
                    /* proc_macro_error::emit_warning!(name.span(), "The view macro is assuming this is an HTML element, \
                    but it is ambiguous; if it is an SVG or MathML element, prefix with svg:: or math::"); */
                    quote! {
                        leptos::leptos_dom::#name(#cx)
                    }
                }
                TagType::Html => quote! { leptos::leptos_dom::#name(#cx) },
                TagType::Svg => quote! { leptos::leptos_dom::svg::#name(#cx) },
                TagType::Math => quote! { leptos::leptos_dom::math::#name(#cx) },
            }
        } else {
            let name = &node.name;
            parent_type = TagType::Html;
            quote! { leptos::leptos_dom::#name(#cx) }
        };
        let attrs = node.attributes.iter().filter_map(|node| {
            if let Node::Attribute(node) = node {
                Some(attribute_to_tokens(cx, node))
            } else {
                None
            }
        });
        let children = node.children.iter().map(|node| {
            let child = match node {
                Node::Fragment(fragment) => fragment_to_tokens(
                    cx,
                    Span::call_site(),
                    &fragment.children,
                    false,
                    parent_type,
                ),
                Node::Text(node) => {
                    let value = node.value.as_ref();
                    quote! {
                        #[allow(unused_braces)] #value
                    }
                }
                Node::Block(node) => {
                    let value = node.value.as_ref();
                    quote! {
                        #[allow(unused_braces)] #value
                    }
                }
                Node::Element(node) => element_to_tokens(cx, node, parent_type),
                Node::Comment(_) | Node::Doctype(_) | Node::Attribute(_) => quote! {},
            };
            quote! {
                .child((#cx, #child))
            }
        });
        quote! {
            #name
                #(#attrs)*
                #(#children)*
        }
    }
}

fn attribute_to_tokens(cx: &Ident, node: &NodeAttribute) -> TokenStream {
    let span = node.key.span();
    let name = node.key.to_string();
    if name == "ref" || name == "_ref" || name == "node_ref" {
        let value = node
            .value
            .as_ref()
            .and_then(|expr| expr_to_ident(expr))
            .expect("'_ref' needs to be passed a variable name");
        let node_ref = quote_spanned! { span => node_ref };

        quote! {
            .#node_ref(&#value)
        }
    } else if let Some(name) = name.strip_prefix("on:") {
        let handler = node
            .value
            .as_ref()
            .expect("event listener attributes need a value")
            .as_ref();

        let (name, is_force_undelegated) = parse_event(name);

        let event_type = TYPED_EVENTS
            .iter()
            .find(|e| **e == name)
            .copied()
            .unwrap_or("Custom");
        let is_custom = event_type == "Custom";
        let event_type = event_type
            .parse::<TokenStream>()
            .expect("couldn't parse event name");

        let event_type = if is_custom {
            quote! { Custom::new(#name) }
        } else {
            event_type
        };

        let event_name_ident = match &node.key {
            NodeName::Punctuated(parts) => {
                if parts.len() >= 2 {
                    Some(&parts[1])
                } else {
                    None
                }
            }
            _ => unreachable!(),
        };
        let undelegated_ident = match &node.key {
            NodeName::Punctuated(parts) => parts.last().and_then(|last| {
                if last == "undelegated" {
                    Some(last)
                } else {
                    None
                }
            }),
            _ => unreachable!(),
        };
        let on = match &node.key {
            NodeName::Punctuated(parts) => &parts[0],
            _ => unreachable!(),
        };
        let on = {
            let span = on.span();
            quote_spanned! {
                span => .on
            }
        };
        let event_type = if is_custom {
            event_type
        } else if let Some(ev_name) = event_name_ident {
            let span = ev_name.span();
            quote_spanned! {
                span => #ev_name
            }
        } else {
            event_type
        };

        let event_type = if is_force_undelegated {
            let undelegated = if let Some(undelegated) = undelegated_ident {
                let span = undelegated.span();
                quote_spanned! {
                    span => #undelegated
                }
            } else {
                quote! { undelegated }
            };
            quote! { ::leptos::ev::#undelegated(::leptos::ev::#event_type) }
        } else {
            quote! { ::leptos::ev::#event_type }
        };

        quote! {
            #on(#event_type, #handler)
        }
    } else if let Some(name) = name.strip_prefix("prop:") {
        let value = node
            .value
            .as_ref()
            .expect("prop: attributes need a value")
            .as_ref();
        let prop = match &node.key {
            NodeName::Punctuated(parts) => &parts[0],
            _ => unreachable!(),
        };
        let prop = {
            let span = prop.span();
            quote_spanned! {
                span => .prop
            }
        };
        quote! {
            #prop(#name, (#cx, #[allow(unused_braces)] #value))
        }
    } else if let Some(name) = name.strip_prefix("class:") {
        let value = node
            .value
            .as_ref()
            .expect("class: attributes need a value")
            .as_ref();
        let class = match &node.key {
            NodeName::Punctuated(parts) => &parts[0],
            _ => unreachable!(),
        };
        let class = {
            let span = class.span();
            quote_spanned! {
                span => .class
            }
        };
        quote! {
            #class(#name, (#cx, #[allow(unused_braces)] #value))
        }
    } else {
        let name = name.replacen("attr:", "", 1);

        if let Some((fancy, _, _)) = fancy_class_name(&name, cx, node) {
            return fancy;
        }

        // all other attributes
        let value = match node.value.as_ref() {
            Some(value) => {
                let value = value.as_ref();

                quote! { #value }
            }
            None => quote_spanned! { span => "" },
        };
        let attr = match &node.key {
            NodeName::Punctuated(parts) => Some(&parts[0]),
            _ => None,
        };
        let attr = if let Some(attr) = attr {
            let span = attr.span();
            quote_spanned! {
                span => .attr
            }
        } else {
            quote! {
                .attr
            }
        };
        quote! {
            #attr(#name, (#cx, #value))
        }
    }
}

fn component_to_tokens(cx: &Ident, node: &NodeElement) -> TokenStream {
    let name = &node.name;
    let component_name = ident_from_tag_name(&node.name);
    let span = node.name.span();
    let component_props_name = format_ident!("{component_name}Props");

    let attrs = node.attributes.iter().filter_map(|node| {
        if let Node::Attribute(node) = node {
            Some(node)
        } else {
            None
        }
    });

    let props = attrs
        .clone()
        .filter(|attr| !attr.key.to_string().starts_with("clone:"))
        .map(|attr| {
            let name = &attr.key;

            let value = attr
                .value
                .as_ref()
                .map(|v| {
                    let v = v.as_ref();
                    quote! { #v }
                })
                .unwrap_or_else(|| quote! { #name });

            quote! {
                .#name(#[allow(unused_braces)] #value)
            }
        });

    let items_to_clone = attrs
        .filter(|attr| attr.key.to_string().starts_with("clone:"))
        .map(|attr| {
            let ident = attr
                .key
                .to_string()
                .strip_prefix("clone:")
                .unwrap()
                .to_owned();

            format_ident!("{ident}", span = attr.key.span())
        })
        .collect::<Vec<_>>();

    let children = if node.children.is_empty() {
        quote! {}
    } else {
        let children = fragment_to_tokens(cx, span, &node.children, true, TagType::Unknown);

        let clonables = items_to_clone
            .iter()
            .map(|ident| quote! { let #ident = #ident.clone(); });

        quote! {
            .children({
                #(#clonables)*

                Box::new(move |#cx| #children)
            })
        }
    };

    quote! {
        #name(
            #cx,
            #component_props_name::builder()
                #(#props)*
                #children
                .build(),
        )
    }
}

fn ident_from_tag_name(tag_name: &NodeName) -> Ident {
    match tag_name {
        NodeName::Path(path) => path
            .path
            .segments
            .iter()
            .last()
            .map(|segment| segment.ident.clone())
            .expect("element needs to have a name"),
        NodeName::Block(_) => {
            let span = tag_name.span();
            proc_macro_error::emit_error!(span, "blocks not allowed in tag-name position");
            Ident::new("", span)
        }
        _ => Ident::new(
            &tag_name.to_string().replace(['-', ':'], "_"),
            tag_name.span(),
        ),
    }
}

fn expr_to_ident(expr: &syn::Expr) -> Option<&ExprPath> {
    match expr {
        syn::Expr::Block(block) => block.block.stmts.last().and_then(|stmt| {
            if let syn::Stmt::Expr(expr) = stmt {
                expr_to_ident(expr)
            } else {
                None
            }
        }),
        syn::Expr::Path(path) => Some(path),
        _ => None,
    }
}

fn is_custom_element(tag: &str) -> bool {
    tag.contains('-')
}

fn is_self_closing(node: &NodeElement) -> bool {
    // self-closing tags
    // https://developer.mozilla.org/en-US/docs/Glossary/Empty_element
    matches!(
        node.name.to_string().as_str(),
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

fn camel_case_tag_name(tag_name: &str) -> String {
    let mut chars = tag_name.chars();
    let first = chars.next();
    first
        .map(|f| f.to_ascii_uppercase())
        .into_iter()
        .chain(chars)
        .collect()
}

fn is_svg_element(tag: &str) -> bool {
    matches!(
        tag,
        "animate"
            | "animateMotion"
            | "animateTransform"
            | "circle"
            | "clipPath"
            | "defs"
            | "desc"
            | "discard"
            | "ellipse"
            | "feBlend"
            | "feColorMatrix"
            | "feComponentTransfer"
            | "feComposite"
            | "feConvolveMatrix"
            | "feDiffuseLighting"
            | "feDisplacementMap"
            | "feDistantLight"
            | "feDropShadow"
            | "feFlood"
            | "feFuncA"
            | "feFuncB"
            | "feFuncG"
            | "feFuncR"
            | "feGaussianBlur"
            | "feImage"
            | "feMerge"
            | "feMergeNode"
            | "feMorphology"
            | "feOffset"
            | "fePointLight"
            | "feSpecularLighting"
            | "feSpotLight"
            | "feTile"
            | "feTurbulence"
            | "filter"
            | "foreignObject"
            | "g"
            | "hatch"
            | "hatchpath"
            | "image"
            | "line"
            | "linearGradient"
            | "marker"
            | "mask"
            | "metadata"
            | "mpath"
            | "path"
            | "pattern"
            | "polygon"
            | "polyline"
            | "radialGradient"
            | "rect"
            | "set"
            | "stop"
            | "svg"
            | "switch"
            | "symbol"
            | "text"
            | "textPath"
            | "tspan"
            | "use"
            | "use_"
            | "view"
    )
}

fn is_math_ml_element(tag: &str) -> bool {
    matches!(
        tag,
        "math"
            | "mi"
            | "mn"
            | "mo"
            | "ms"
            | "mspace"
            | "mtext"
            | "menclose"
            | "merror"
            | "mfenced"
            | "mfrac"
            | "mpadded"
            | "mphantom"
            | "mroot"
            | "mrow"
            | "msqrt"
            | "mstyle"
            | "mmultiscripts"
            | "mover"
            | "mprescripts"
            | "msub"
            | "msubsup"
            | "msup"
            | "munder"
            | "munderover"
            | "mtable"
            | "mtd"
            | "mtr"
            | "maction"
            | "annotation"
            | "semantics"
    )
}

fn is_ambiguous_element(tag: &str) -> bool {
    tag == "a" || tag == "script"
}

fn parse_event(event_name: &str) -> (&str, bool) {
    if let Some(event_name) = event_name.strip_suffix(":undelegated") {
        (event_name, true)
    } else {
        (event_name, false)
    }
}

fn fancy_class_name<'a>(
    name: &str,
    cx: &Ident,
    node: &'a NodeAttribute,
) -> Option<(TokenStream, String, &'a Expr)> {
    // special case for complex class names:
    // e.g., Tailwind `class=("mt-[calc(100vh_-_3rem)]", true)`
    if name == "class" {
        if let Some(expr) = node.value.as_ref() {
            if let syn::Expr::Tuple(tuple) = expr.as_ref() {
                if tuple.elems.len() == 2 {
                    let span = node.key.span();
                    let class = quote_spanned! {
                        span => .class
                    };
                    let class_name = &tuple.elems[0];
                    let class_name = if let Expr::Lit(ExprLit {
                        lit: Lit::Str(s), ..
                    }) = class_name
                    {
                        s.value()
                    } else {
                        proc_macro_error::emit_error!(
                            class_name.span(),
                            "class name must be a string literal"
                        );
                        Default::default()
                    };
                    let value = &tuple.elems[1];
                    return Some((
                        quote! {
                            #class(#class_name, (#cx, #value))
                        },
                        class_name,
                        value,
                    ));
                } else {
                    proc_macro_error::emit_error!(
                        tuple.span(),
                        "class tuples must have two elements."
                    )
                }
            }
        }
    }
    None
}
