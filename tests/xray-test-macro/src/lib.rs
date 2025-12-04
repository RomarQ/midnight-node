use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::parse_quote;
use syn::punctuated::Punctuated;
use syn::{ItemFn, LitStr, Result, Token, parenthesized, parse_macro_input};

struct XrayTest {
	key: Option<LitStr>,
	stories: Vec<LitStr>,
	labels: Vec<LitStr>,
}

impl Parse for XrayTest {
	fn parse(input: ParseStream) -> Result<Self> {
		let mut key: Option<LitStr> = None;
		let mut stories: Vec<LitStr> = Vec::new();
		let mut labels: Vec<LitStr> = Vec::new();

		while !input.is_empty() {
			let ident: Ident = input.parse()?;
			let name = ident.to_string();

			if name == "key" {
				input.parse::<Token![=]>()?;
				let lit: LitStr = input.parse()?;
				key = Some(lit);
			} else if name == "stories" {
				let content;
				parenthesized!(content in input);

				let items: Punctuated<LitStr, Token![,]> =
					content.parse_terminated(<LitStr as Parse>::parse, Token![,])?;
				stories.extend(items.into_iter());
			} else if name == "labels" {
				let content;
				parenthesized!(content in input);

				let items: Punctuated<LitStr, Token![,]> =
					content.parse_terminated(<LitStr as Parse>::parse, Token![,])?;
				labels.extend(items.into_iter());
			} else {
				return Err(syn::Error::new(
					ident.span(),
					"xray_test: expected `key`, `stories` or `labels`",
				));
			}

			if input.peek(Token![,]) {
				let _ = input.parse::<Token![,]>()?;
			}
		}

		Ok(XrayTest { key, stories, labels })
	}
}

/**
Example usage: #[xray_test(key = "PM-20691", stories("PM-20459"), labels("ledger"))]
*/
#[proc_macro_attribute]
pub fn xray_test(attr: TokenStream, item: TokenStream) -> TokenStream {
	let args = parse_macro_input!(attr as XrayTest);
	let mut func = parse_macro_input!(item as ItemFn);

	let fn_name = &func.sig.ident;
	let test_key_lit =
		args.key.unwrap_or_else(|| LitStr::new(&fn_name.to_string(), fn_name.span()));

	let stories_str: String = args.stories.iter().map(|s| s.value()).collect::<Vec<_>>().join(",");
	let stories_lit = LitStr::new(&stories_str, fn_name.span());

	let labels_str: String = args.labels.iter().map(|s| s.value()).collect::<Vec<_>>().join(",");
	let labels_lit = LitStr::new(&labels_str, fn_name.span());

	let orig_block = &func.block;

	func.block = parse_quote!({
		println!("### XRAY_TEST KEY:{} STORIES:{} LABELS: {} ###", #test_key_lit, #stories_lit, #labels_lit);
		#orig_block
	});

	TokenStream::from(quote!(#func))
}
