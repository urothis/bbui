//! Rust macros for declaring real BBUI widget trees.

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Expr, ExprArray, Ident, LitBool, LitStr, Result, Token, braced, parenthesized};

#[proc_macro]
pub fn bbui(input: TokenStream) -> TokenStream {
    let declaration = syn::parse_macro_input!(input as WidgetDecl);
    let root = declaration.build();
    quote!(::bbui::widgets::WidgetTree::new(#root)).into()
}

enum WidgetDecl {
    Row(Vec<WidgetDecl>),
    Column(Vec<WidgetDecl>),
    MenuBar(Vec<WidgetDecl>),
    Overlay(Vec<WidgetDecl>),
    Inline(Vec<WidgetDecl>),
    Boxed(Vec<WidgetDecl>),
    Scroll(Vec<WidgetDecl>),
    Grid(Box<Expr>, Vec<WidgetDecl>),
    ResponsiveGrid(Box<Expr>, Box<Expr>, Vec<WidgetDecl>),
    FlexRow(Box<Expr>, Vec<WidgetDecl>),
    FlexColumn(Box<Expr>, Vec<WidgetDecl>),
    PropertyRow(LitStr, Box<WidgetDecl>),
    Tooltip(LitStr, Box<WidgetDecl>),
    Label(LitStr),
    Button(LitStr),
    IconButton(LitStr, LitStr),
    IconButtonAsset(LitStr, LitStr),
    Checkbox(LitStr, LitBool),
    RadioGroup(LitStr, Box<Expr>),
    Number(LitStr, Box<Expr>, Box<Expr>, Box<Expr>, Box<Expr>),
    UnboundedNumber(LitStr, Box<Expr>, Box<Expr>),
    Slider(LitStr, Box<Expr>),
    SliderRange(LitStr, Box<Expr>, Box<Expr>, Box<Expr>, Box<Expr>),
    IntegerSliderRange(LitStr, Box<Expr>, Box<Expr>, Box<Expr>, Box<Expr>),
    Panel(LitStr, LitBool, Vec<WidgetDecl>),
    TextField(LitStr, LitStr),
    TextArea(LitStr, LitStr),
    Dropdown(LitStr, Vec<LitStr>, Box<Expr>),
    Menu(LitStr, Box<Expr>),
    PieMenu(LitStr, Box<Expr>),
    Popover(LitStr, Box<Expr>),
    ContextMenu(LitStr, Box<Expr>),
    Progress(Box<Expr>),
    FileBrowser(Box<Expr>),
    ColorRamp(LitStr, Box<Expr>),
    ColorRampDefault(LitStr),
    HsvColor(LitStr, Box<Expr>),
    HsvColorDefault(LitStr),
    CurveEditor(LitStr, Box<Expr>),
    CurveEditorDefault(LitStr),
    List(LitStr, Box<Expr>),
    ListDefault(LitStr),
    Tree(LitStr, Box<Expr>),
    TreeDefault(LitStr),
    PreviewGrid(LitStr, Box<Expr>),
    PreviewGridDefault(LitStr),
    DataTable(LitStr, Box<Expr>),
    DataTableDefault(LitStr),
    NodeGraph(LitStr, Box<Expr>),
    NodeGraphDefault(LitStr),
    Canvas(LitStr, Box<Expr>),
    CanvasDefault(LitStr),
}

impl Parse for WidgetDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let kind: Ident = input.parse()?;
        let name = kind.to_string();
        if matches!(
            name.as_str(),
            "row" | "column" | "menu_bar" | "overlay" | "inline" | "boxed" | "scroll"
        ) {
            let content;
            braced!(content in input);
            let children = Punctuated::<WidgetDecl, Token![,]>::parse_terminated(&content)?
                .into_iter()
                .collect();
            return Ok(match name.as_str() {
                "row" => Self::Row(children),
                "column" => Self::Column(children),
                "menu_bar" => Self::MenuBar(children),
                "overlay" => Self::Overlay(children),
                "inline" => Self::Inline(children),
                "boxed" => Self::Boxed(children),
                "scroll" => Self::Scroll(children),
                _ => return Err(syn::Error::new(kind.span(), "unsupported container")),
            });
        }
        if matches!(
            name.as_str(),
            "grid" | "responsive_grid" | "flex_row" | "flex_column"
        ) {
            let args;
            parenthesized!(args in input);
            let first: Expr = args.parse()?;
            let second = if name == "responsive_grid" {
                args.parse::<Token![,]>()?;
                Some(args.parse::<Expr>()?)
            } else {
                None
            };
            if !args.is_empty() {
                return Err(args.error("unexpected extra layout arguments"));
            }
            let content;
            braced!(content in input);
            let children = Punctuated::<WidgetDecl, Token![,]>::parse_terminated(&content)?
                .into_iter()
                .collect();
            return Ok(match name.as_str() {
                "grid" => Self::Grid(Box::new(first), children),
                "responsive_grid" => Self::ResponsiveGrid(
                    Box::new(first),
                    Box::new(
                        second
                            .ok_or_else(|| args.error("responsive grid requires two arguments"))?,
                    ),
                    children,
                ),
                "flex_row" => Self::FlexRow(Box::new(first), children),
                "flex_column" => Self::FlexColumn(Box::new(first), children),
                _ => return Err(syn::Error::new(kind.span(), "unsupported layout")),
            });
        }
        if name == "property_row" {
            let args;
            parenthesized!(args in input);
            let label = args.parse()?;
            if !args.is_empty() {
                return Err(args.error("unexpected extra property row arguments"));
            }
            let child;
            braced!(child in input);
            let mut children = Punctuated::<WidgetDecl, Token![,]>::parse_terminated(&child)?;
            if children.len() != 1 {
                return Err(syn::Error::new(
                    kind.span(),
                    "property_row requires exactly one control",
                ));
            }
            let child = children
                .pop()
                .ok_or_else(|| child.error("property_row requires a control"))?;
            return Ok(Self::PropertyRow(label, Box::new(child.into_value())));
        }
        if name == "tooltip" {
            let args;
            parenthesized!(args in input);
            let text = args.parse()?;
            if !args.is_empty() {
                return Err(args.error("unexpected extra tooltip arguments"));
            }
            let child;
            braced!(child in input);
            let mut children = Punctuated::<WidgetDecl, Token![,]>::parse_terminated(&child)?;
            if children.len() != 1 {
                return Err(syn::Error::new(
                    kind.span(),
                    "tooltip requires exactly one child widget",
                ));
            }
            let child = children
                .pop()
                .ok_or_else(|| child.error("tooltip requires a child"))?;
            return Ok(Self::Tooltip(text, Box::new(child.into_value())));
        }

        let args;
        parenthesized!(args in input);
        let declaration = match name.as_str() {
            "label" => Self::Label(args.parse()?),
            "button" => Self::Button(args.parse()?),
            "icon_button" => {
                let icon = args.parse()?;
                args.parse::<Token![,]>()?;
                Self::IconButton(icon, args.parse()?)
            }
            "icon_button_asset" => {
                let path = args.parse()?;
                args.parse::<Token![,]>()?;
                Self::IconButtonAsset(path, args.parse()?)
            }
            "checkbox" => {
                let label = args.parse()?;
                args.parse::<Token![,]>()?;
                Self::Checkbox(label, args.parse()?)
            }
            "radio_group" => {
                let label = args.parse()?;
                args.parse::<Token![,]>()?;
                Self::RadioGroup(label, Box::new(args.parse()?))
            }
            "number" => {
                let label = args.parse()?;
                args.parse::<Token![,]>()?;
                let value = args.parse()?;
                args.parse::<Token![,]>()?;
                let min = args.parse()?;
                args.parse::<Token![,]>()?;
                let max = args.parse()?;
                args.parse::<Token![,]>()?;
                let step = args.parse()?;
                Self::Number(
                    label,
                    Box::new(value),
                    Box::new(min),
                    Box::new(max),
                    Box::new(step),
                )
            }
            "number_unbounded" => {
                let label = args.parse()?;
                args.parse::<Token![,]>()?;
                let value = args.parse()?;
                args.parse::<Token![,]>()?;
                let step = args.parse()?;
                Self::UnboundedNumber(label, Box::new(value), Box::new(step))
            }
            "slider" => {
                let label = args.parse()?;
                args.parse::<Token![,]>()?;
                Self::Slider(label, Box::new(args.parse()?))
            }
            "slider_range" | "integer_slider_range" => {
                let label = args.parse()?;
                args.parse::<Token![,]>()?;
                let value = Box::new(args.parse()?);
                args.parse::<Token![,]>()?;
                let min = Box::new(args.parse()?);
                args.parse::<Token![,]>()?;
                let max = Box::new(args.parse()?);
                args.parse::<Token![,]>()?;
                let step = Box::new(args.parse()?);
                if name == "slider_range" {
                    Self::SliderRange(label, value, min, max, step)
                } else {
                    Self::IntegerSliderRange(label, value, min, max, step)
                }
            }
            "panel" => {
                let title = args.parse()?;
                let expanded = if args.peek(Token![,]) {
                    args.parse::<Token![,]>()?;
                    args.parse()?
                } else {
                    LitBool::new(true, kind.span())
                };
                let children;
                braced!(children in input);
                let children = Punctuated::<WidgetDecl, Token![,]>::parse_terminated(&children)?
                    .into_iter()
                    .collect();
                Self::Panel(title, expanded, children)
            }
            "text_field" => {
                let label = args.parse()?;
                args.parse::<Token![,]>()?;
                Self::TextField(label, args.parse()?)
            }
            "text_area" => {
                let label = args.parse()?;
                args.parse::<Token![,]>()?;
                Self::TextArea(label, args.parse()?)
            }
            "dropdown" => {
                let label = args.parse()?;
                args.parse::<Token![,]>()?;
                let options: ExprArray = args.parse()?;
                args.parse::<Token![,]>()?;
                let selected: Expr = args.parse()?;
                let values = options
                    .elems
                    .into_iter()
                    .map(|expr| {
                        let span = expr.span();
                        match expr {
                            Expr::Lit(lit) => match lit.lit {
                                syn::Lit::Str(text) => Ok(text),
                                _ => Err(syn::Error::new(
                                    span,
                                    "dropdown options must be string literals",
                                )),
                            },
                            _ => Err(syn::Error::new(
                                span,
                                "dropdown options must be string literals",
                            )),
                        }
                    })
                    .collect::<Result<Vec<_>>>()?;
                Self::Dropdown(label, values, Box::new(selected))
            }
            "menu" | "pie_menu" | "popover" | "context_menu" => {
                let label = args.parse()?;
                args.parse::<Token![,]>()?;
                let entries = Box::new(args.parse()?);
                match name.as_str() {
                    "menu" => Self::Menu(label, entries),
                    "pie_menu" => Self::PieMenu(label, entries),
                    "popover" => Self::Popover(label, entries),
                    _ => Self::ContextMenu(label, entries),
                }
            }
            "progress" => Self::Progress(Box::new(args.parse()?)),
            "file_browser" => Self::FileBrowser(Box::new(args.parse()?)),
            "color_ramp" => {
                let label = args.parse()?;
                args.parse::<Token![,]>()?;
                Self::ColorRamp(label, Box::new(args.parse()?))
            }
            "color_ramp_default" => Self::ColorRampDefault(args.parse()?),
            "hsv_color" => {
                let label = args.parse()?;
                args.parse::<Token![,]>()?;
                Self::HsvColor(label, Box::new(args.parse()?))
            }
            "hsv_color_default" => Self::HsvColorDefault(args.parse()?),
            "curve_editor" => {
                let label = args.parse()?;
                args.parse::<Token![,]>()?;
                Self::CurveEditor(label, Box::new(args.parse()?))
            }
            "curve_editor_default" => Self::CurveEditorDefault(args.parse()?),
            "list" => {
                let label = args.parse()?;
                args.parse::<Token![,]>()?;
                Self::List(label, Box::new(args.parse()?))
            }
            "list_default" => Self::ListDefault(args.parse()?),
            "tree" => {
                let label = args.parse()?;
                args.parse::<Token![,]>()?;
                Self::Tree(label, Box::new(args.parse()?))
            }
            "tree_default" => Self::TreeDefault(args.parse()?),
            "preview_grid" => {
                let label = args.parse()?;
                args.parse::<Token![,]>()?;
                Self::PreviewGrid(label, Box::new(args.parse()?))
            }
            "preview_grid_default" => Self::PreviewGridDefault(args.parse()?),
            "data_table" => {
                let label = args.parse()?;
                args.parse::<Token![,]>()?;
                Self::DataTable(label, Box::new(args.parse()?))
            }
            "data_table_default" => Self::DataTableDefault(args.parse()?),
            "node_graph" => {
                let label = args.parse()?;
                args.parse::<Token![,]>()?;
                Self::NodeGraph(label, Box::new(args.parse()?))
            }
            "node_graph_default" => Self::NodeGraphDefault(args.parse()?),
            "canvas" => {
                let label = args.parse()?;
                args.parse::<Token![,]>()?;
                Self::Canvas(label, Box::new(args.parse()?))
            }
            "canvas_default" => Self::CanvasDefault(args.parse()?),
            _ => return Err(syn::Error::new_spanned(kind, "unknown widget declaration")),
        };
        if !args.is_empty() {
            return Err(args.error("unexpected extra widget arguments"));
        }
        Ok(declaration)
    }
}

impl WidgetDecl {
    fn build(self) -> proc_macro2::TokenStream {
        match self {
            Self::Row(children) => {
                let children = children.into_iter().map(Self::build);
                quote!(::bbui::widgets::WidgetNode::row(vec![#(#children),*]))
            }
            Self::Column(children) => {
                let children = children.into_iter().map(Self::build);
                quote!(::bbui::widgets::WidgetNode::column(vec![#(#children),*]))
            }
            Self::MenuBar(children) => {
                let children = children.into_iter().map(Self::build);
                quote!(::bbui::widgets::WidgetNode::menu_bar(vec![#(#children),*]))
            }
            Self::Overlay(children) => {
                let children = children.into_iter().map(Self::build);
                quote!(::bbui::widgets::WidgetNode::overlay(vec![#(#children),*]))
            }
            Self::Inline(children) => {
                let children = children.into_iter().map(Self::build);
                quote!(::bbui::widgets::WidgetNode::inline(vec![#(#children),*]))
            }
            Self::Boxed(children) => {
                let children = children.into_iter().map(Self::build);
                quote!(::bbui::widgets::WidgetNode::boxed(vec![#(#children),*]))
            }
            Self::Scroll(children) => {
                let children = children.into_iter().map(Self::build);
                quote!(::bbui::widgets::WidgetNode::scroll(vec![#(#children),*]))
            }
            Self::Grid(columns, children) => {
                let children = children.into_iter().map(Self::build);
                quote!(::bbui::widgets::WidgetNode::grid(usize::try_from((#columns)).unwrap_or(0), vec![#(#children),*]))
            }
            Self::ResponsiveGrid(min_cell_width, max_columns, children) => {
                let children = children.into_iter().map(Self::build);
                quote!(::bbui::widgets::WidgetNode::responsive_grid(u32::try_from((#min_cell_width)).unwrap_or(0), usize::try_from((#max_columns)).unwrap_or(0), vec![#(#children),*]))
            }
            Self::FlexRow(wrap, children) => {
                let children = children.into_iter().map(Self::build);
                quote!(::bbui::widgets::WidgetNode::flex(::bbui::prelude::UiFlexDirection::Row, #wrap, vec![#(#children),*]))
            }
            Self::FlexColumn(wrap, children) => {
                let children = children.into_iter().map(Self::build);
                quote!(::bbui::widgets::WidgetNode::flex(::bbui::prelude::UiFlexDirection::Column, #wrap, vec![#(#children),*]))
            }
            Self::PropertyRow(label, child) => {
                let child = child.build();
                quote!(::bbui::widgets::WidgetNode::property_row(#label, #child))
            }
            Self::Tooltip(text, child) => {
                let child = child.build();
                quote!(::bbui::widgets::WidgetNode::tooltip(#text, #child))
            }
            Self::Label(text) => quote!(::bbui::widgets::WidgetNode::label(#text)),
            Self::Button(label) => quote!(::bbui::widgets::WidgetNode::button(#label)),
            Self::IconButton(icon, accessible_name) => {
                quote!(::bbui::widgets::WidgetNode::icon_button(#icon, #accessible_name))
            }
            Self::IconButtonAsset(path, accessible_name) => {
                quote!(::bbui::widgets::WidgetNode::icon_button_asset(#path, #accessible_name))
            }
            Self::Checkbox(label, checked) => {
                quote!(::bbui::widgets::WidgetNode::checkbox(#label, #checked))
            }
            Self::RadioGroup(label, group) => {
                quote!(::bbui::widgets::WidgetNode::radio_group(#label, (#group)))
            }
            Self::Number(label, value, min, max, step) => {
                quote!(::bbui::widgets::WidgetNode::number(#label, (#value) as f64, (#min) as f64, (#max) as f64, (#step) as f64))
            }
            Self::UnboundedNumber(label, value, step) => {
                quote!(::bbui::widgets::WidgetNode::unbounded_number(#label, (#value) as f64, (#step) as f64))
            }
            Self::Slider(label, slider) => {
                quote!(::bbui::widgets::WidgetNode::slider(#label, (#slider)))
            }
            Self::SliderRange(label, value, min, max, step) => {
                quote!(::bbui::widgets::WidgetNode::slider_range(#label, (#value) as f64, (#min) as f64, (#max) as f64, (#step) as f64))
            }
            Self::IntegerSliderRange(label, value, min, max, step) => {
                quote!(::bbui::widgets::WidgetNode::integer_slider_range(#label, (#value) as f64, (#min) as f64, (#max) as f64, (#step) as f64))
            }
            Self::Panel(title, expanded, children) => {
                let children = children.into_iter().map(Self::build);
                quote!(::bbui::widgets::WidgetNode::panel(#title, #expanded, vec![#(#children),*]))
            }
            Self::TextField(label, value) => {
                quote!(::bbui::widgets::WidgetNode::text_field(#label, #value))
            }
            Self::TextArea(label, value) => {
                quote!(::bbui::widgets::WidgetNode::text_area(#label, #value))
            }
            Self::Dropdown(label, options, selected) => {
                quote!(::bbui::widgets::WidgetNode::dropdown(#label, [#(#options),*], #selected))
            }
            Self::Menu(label, entries) => {
                quote!(::bbui::widgets::WidgetNode::menu(#label, (#entries)))
            }
            Self::PieMenu(label, entries) => {
                quote!(::bbui::widgets::WidgetNode::pie_menu(#label, (#entries)))
            }
            Self::Popover(label, entries) => {
                quote!(::bbui::widgets::WidgetNode::popover(#label, (#entries)))
            }
            Self::ContextMenu(label, entries) => {
                quote!(::bbui::widgets::WidgetNode::context_menu(#label, (#entries)))
            }
            Self::Progress(value) => quote!(::bbui::widgets::WidgetNode::progress((#value) as f32)),
            Self::FileBrowser(path) => {
                quote!(::bbui::widgets::WidgetNode::file_browser(#path))
            }
            Self::ColorRamp(label, ramp) => {
                quote!(::bbui::widgets::WidgetNode::color_ramp(#label, (#ramp)))
            }
            Self::ColorRampDefault(label) => {
                quote!(::bbui::widgets::WidgetNode::color_ramp_default(#label))
            }
            Self::HsvColor(label, color) => {
                quote!(::bbui::widgets::WidgetNode::hsv_color(#label, (#color)))
            }
            Self::HsvColorDefault(label) => {
                quote!(::bbui::widgets::WidgetNode::hsv_color_default(#label))
            }
            Self::CurveEditor(label, editor) => {
                quote!(::bbui::widgets::WidgetNode::curve_editor(#label, (#editor)))
            }
            Self::CurveEditorDefault(label) => {
                quote!(::bbui::widgets::WidgetNode::curve_editor_default(#label))
            }
            Self::List(label, model) => {
                quote!(::bbui::widgets::WidgetNode::list(#label, (#model)))
            }
            Self::ListDefault(label) => {
                quote!(::bbui::widgets::WidgetNode::list_default(#label))
            }
            Self::Tree(label, model) => {
                quote!(::bbui::widgets::WidgetNode::tree(#label, (#model)))
            }
            Self::TreeDefault(label) => {
                quote!(::bbui::widgets::WidgetNode::tree_default(#label))
            }
            Self::PreviewGrid(label, model) => {
                quote!(::bbui::widgets::WidgetNode::preview_grid(#label, (#model)))
            }
            Self::PreviewGridDefault(label) => {
                quote!(::bbui::widgets::WidgetNode::preview_grid_default(#label))
            }
            Self::DataTable(label, model) => {
                quote!(::bbui::widgets::WidgetNode::data_table(#label, (#model)))
            }
            Self::DataTableDefault(label) => {
                quote!(::bbui::widgets::WidgetNode::data_table_default(#label))
            }
            Self::NodeGraph(label, model) => {
                quote!(::bbui::widgets::WidgetNode::node_graph(#label, (#model)))
            }
            Self::NodeGraphDefault(label) => {
                quote!(::bbui::widgets::WidgetNode::node_graph_default(#label))
            }
            Self::Canvas(label, model) => {
                quote!(::bbui::widgets::WidgetNode::canvas(#label, (#model)))
            }
            Self::CanvasDefault(label) => {
                quote!(::bbui::widgets::WidgetNode::canvas_default(#label))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::ToTokens;

    #[test]
    fn nested_declarations_expand_to_a_widget_tree_constructor() {
        let input: proc_macro2::TokenStream = quote! {
            column {
                label("Transform"),
                checkbox("Enabled", true),
                panel("Advanced", false) {
                    number("Scale", 1.0, 0.0, 10.0, 0.1),
                    text_field("Name", "Cube"),
                },
            }
        };
        let declaration = syn::parse2::<WidgetDecl>(input).unwrap();
        let built = declaration.build();
        let expanded = quote!(::bbui::widgets::WidgetTree::new(#built));
        let parsed: syn::Expr = syn::parse2(expanded.clone()).unwrap();
        assert!(parsed.to_token_stream().to_string().contains("WidgetTree"));
    }

    #[test]
    fn dropdown_requires_literal_options_and_accepts_selected_index_expression() {
        let input: proc_macro2::TokenStream =
            quote! { dropdown("Mode", ["Object", "Edit"], Some(1)) };
        assert!(syn::parse2::<WidgetDecl>(input).is_ok());
    }

    #[test]
    fn radio_and_icon_controls_accept_model_and_accessible_name() {
        let radio = syn::parse2::<WidgetDecl>(quote! {
            radio_group("Mode", group.clone())
        })
        .unwrap();
        assert!(
            radio
                .build()
                .to_string()
                .contains("WidgetNode :: radio_group")
        );
        let icon = syn::parse2::<WidgetDecl>(quote! {
            icon_button("+", "Add object")
        })
        .unwrap();
        assert!(
            icon.build()
                .to_string()
                .contains("WidgetNode :: icon_button")
        );
        let icon_asset = syn::parse2::<WidgetDecl>(quote! {
            icon_button_asset("icons/add.svg", "Add object")
        })
        .unwrap();
        assert!(icon_asset.build().to_string().contains("icon_button_asset"));
    }

    #[test]
    fn slider_accepts_a_reusable_numeric_slider_model() {
        let declaration = syn::parse2::<WidgetDecl>(quote! {
            slider("Opacity", slider_model.clone())
        })
        .unwrap();
        assert!(
            declaration
                .build()
                .to_string()
                .contains("WidgetNode :: slider")
        );
    }

    #[test]
    fn slider_range_forms_accept_bounds_and_integer_steps() {
        for (declaration, expected) in [
            (
                syn::parse2::<WidgetDecl>(quote! {
                    slider_range("Opacity", 0.5, 0.0, 1.0, 0.01)
                })
                .unwrap(),
                "WidgetNode :: slider_range",
            ),
            (
                syn::parse2::<WidgetDecl>(quote! {
                    integer_slider_range("Count", 4, 0, 12, 1)
                })
                .unwrap(),
                "WidgetNode :: integer_slider_range",
            ),
        ] {
            assert!(declaration.build().to_string().contains(expected));
        }
    }

    #[test]
    fn layout_containers_and_property_rows_expand_to_public_widget_nodes() {
        let declaration = syn::parse2::<WidgetDecl>(quote! {
            scroll {
                boxed {
                    property_row("Scale") { slider_range("Value", 0.5, 0.0, 1.0, 0.1) },
                    grid(2) { button("A"), button("B") },
                    responsive_grid(96, 4) { button("C") },
                    flex_row(true) { button("D") },
                    flex_column(false) { button("E") },
                    overlay { label("Front"), label("Back") },
                    inline { label("One"), label("Two") },
                }
            }
        })
        .unwrap();
        let expanded = declaration.build().to_string();
        for constructor in [
            "WidgetNode :: scroll",
            "WidgetNode :: boxed",
            "WidgetNode :: property_row",
            "WidgetNode :: grid",
            "WidgetNode :: responsive_grid",
            "WidgetNode :: flex",
            "WidgetNode :: overlay",
            "WidgetNode :: inline",
        ] {
            assert!(expanded.contains(constructor), "missing {constructor}");
        }
        assert!(
            syn::parse2::<WidgetDecl>(quote! {
                property_row("Broken") { button("A"), button("B") }
            })
            .is_err()
        );
        assert!(
            syn::parse2::<WidgetDecl>(quote! {
                flex_row(wrap_from_state) { button("Dynamic") }
            })
            .is_ok()
        );
    }

    #[test]
    fn menus_and_menu_bars_accept_runtime_entry_expressions() {
        for (source, expected) in [
            (
                quote! { menu("File", entries.clone()) },
                "WidgetNode :: menu",
            ),
            (
                quote! { pie_menu("Radial", entries.clone()) },
                "WidgetNode :: pie_menu",
            ),
            (
                quote! { popover("Inspector", entries.clone()) },
                "WidgetNode :: popover",
            ),
            (
                quote! { context_menu("Object", entries.clone()) },
                "WidgetNode :: context_menu",
            ),
        ] {
            let declaration = syn::parse2::<WidgetDecl>(source).unwrap();
            assert!(declaration.build().to_string().contains(expected));
        }

        let declaration = syn::parse2::<WidgetDecl>(quote! {
            menu_bar {
                menu("File", entries.clone()),
                menu("Edit", entries.clone()),
            }
        })
        .unwrap();
        assert!(
            declaration
                .build()
                .to_string()
                .contains("WidgetNode :: menu_bar")
        );
    }

    #[test]
    fn tooltip_wraps_exactly_one_child_widget() {
        let declaration = syn::parse2::<WidgetDecl>(quote! {
            tooltip("Apply changes") {
                button("Apply"),
            }
        })
        .unwrap();
        assert!(
            declaration
                .build()
                .to_string()
                .contains("WidgetNode :: tooltip")
        );
        assert!(
            syn::parse2::<WidgetDecl>(quote! {
                tooltip("bad") { button("One"), button("Two") }
            })
            .is_err()
        );
    }

    #[test]
    fn file_browser_accepts_a_runtime_path_expression() {
        let input: proc_macro2::TokenStream = quote! { file_browser(asset_root.clone()) };
        let declaration = syn::parse2::<WidgetDecl>(input).unwrap();
        assert!(declaration.build().to_string().contains("file_browser"));
    }

    #[test]
    fn color_ramp_accepts_model_or_default_constructor() {
        let explicit: proc_macro2::TokenStream = quote! {
            color_ramp("Gradient", ramp.clone())
        };
        let default: proc_macro2::TokenStream = quote! {
            color_ramp_default("Gradient")
        };
        assert!(
            syn::parse2::<WidgetDecl>(explicit)
                .unwrap()
                .build()
                .to_string()
                .contains("color_ramp")
        );
        assert!(
            syn::parse2::<WidgetDecl>(default)
                .unwrap()
                .build()
                .to_string()
                .contains("color_ramp_default")
        );
    }

    #[test]
    fn hsv_color_accepts_srgb_color_or_default_constructor() {
        let explicit: proc_macro2::TokenStream = quote! {
            hsv_color("Picker", color)
        };
        let default: proc_macro2::TokenStream = quote! {
            hsv_color_default("Picker")
        };
        assert!(
            syn::parse2::<WidgetDecl>(explicit)
                .unwrap()
                .build()
                .to_string()
                .contains("hsv_color")
        );
        assert!(
            syn::parse2::<WidgetDecl>(default)
                .unwrap()
                .build()
                .to_string()
                .contains("hsv_color_default")
        );
    }

    #[test]
    fn curve_editor_accepts_model_or_default_constructor() {
        let explicit: proc_macro2::TokenStream = quote! {
            curve_editor("Ease", curve.clone())
        };
        let default: proc_macro2::TokenStream = quote! {
            curve_editor_default("Ease")
        };
        assert!(
            syn::parse2::<WidgetDecl>(explicit)
                .unwrap()
                .build()
                .to_string()
                .contains("curve_editor")
        );
        assert!(
            syn::parse2::<WidgetDecl>(default)
                .unwrap()
                .build()
                .to_string()
                .contains("curve_editor_default")
        );
    }

    #[test]
    fn collection_widgets_accept_models_or_default_constructors() {
        for (source, expected) in [
            (quote! { list("Objects", model.clone()) }, "list"),
            (quote! { list_default("Objects") }, "list_default"),
            (quote! { tree("Hierarchy", model.clone()) }, "tree"),
            (quote! { tree_default("Hierarchy") }, "tree_default"),
            (
                quote! { preview_grid("Assets", model.clone()) },
                "preview_grid",
            ),
            (
                quote! { preview_grid_default("Assets") },
                "preview_grid_default",
            ),
        ] {
            assert!(
                syn::parse2::<WidgetDecl>(source)
                    .unwrap()
                    .build()
                    .to_string()
                    .contains(expected)
            );
        }
    }

    #[test]
    fn data_table_accepts_model_or_default_constructor() {
        for (source, expected) in [
            (
                quote! { data_table("Inventory", model.clone()) },
                "data_table",
            ),
            (
                quote! { data_table_default("Inventory") },
                "data_table_default",
            ),
        ] {
            assert!(
                syn::parse2::<WidgetDecl>(source)
                    .unwrap()
                    .build()
                    .to_string()
                    .contains(expected)
            );
        }
    }

    #[test]
    fn node_graph_accepts_model_or_default_constructor() {
        for (source, expected) in [
            (quote! { node_graph("Shader", graph.clone()) }, "node_graph"),
            (
                quote! { node_graph_default("Shader") },
                "node_graph_default",
            ),
        ] {
            assert!(
                syn::parse2::<WidgetDecl>(source)
                    .unwrap()
                    .build()
                    .to_string()
                    .contains(expected)
            );
        }
    }

    #[test]
    fn canvas_accepts_model_or_default_constructor() {
        for (source, expected) in [
            (quote! { canvas("Scene", model.clone()) }, "canvas"),
            (quote! { canvas_default("Scene") }, "canvas_default"),
        ] {
            assert!(
                syn::parse2::<WidgetDecl>(source)
                    .unwrap()
                    .build()
                    .to_string()
                    .contains(expected)
            );
        }
    }
}
