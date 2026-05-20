//! `#[derive(QuickcheckArbitrary)]` — bridges `quickcheck::Arbitrary` to a
//! type's existing `arbitrary::Arbitrary` impl. Injected on every
//! buffa-generated type via buffa-build's `type_attribute`, behind the
//! consumer crate's `quickcheck` feature (which implies `arbitrary`).

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(QuickcheckArbitrary)]
pub fn derive_quickcheck_arbitrary(input: TokenStream) -> TokenStream {
  let ast = parse_macro_input!(input as DeriveInput);
  let name = &ast.ident;
  let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

  let expanded = quote! {
      impl #impl_generics ::quickcheck::Arbitrary for #name #ty_generics #where_clause {
          fn arbitrary(g: &mut ::quickcheck::Gen) -> Self {
              // First attempt is 256 bytes, then doubles. For every
              // buffa-generated target type the first attempt succeeds in
              // practice; the zeroed-buffer fallback below is defensive only.
              let mut len: usize = 256;
              for _ in 0..8 {
                  let bytes: ::std::vec::Vec<u8> =
                      (0..len).map(|_| <u8 as ::quickcheck::Arbitrary>::arbitrary(g)).collect();
                  let mut u = ::arbitrary::Unstructured::new(&bytes);
                  if let ::core::result::Result::Ok(v) =
                      <Self as ::arbitrary::Arbitrary>::arbitrary(&mut u)
                  {
                      return v;
                  }
                  len = len.saturating_mul(2);
              }
              let big = ::std::vec![0u8; len];
              let mut u = ::arbitrary::Unstructured::new(&big);
              <Self as ::arbitrary::Arbitrary>::arbitrary(&mut u)
                  .expect("arbitrary::Arbitrary failed even with a large zeroed buffer")
          }
      }
  };
  expanded.into()
}
