mod attr;
mod r#enum;
mod r#struct;

use {
    attr::Attributes,
    proc_macro::TokenStream,
    proc_macro2::TokenStream as TokenStream2,
    quote::quote,
    r#enum::enum_builder,
    r#struct::struct_builder,
    syn::{parse_macro_input, Data, DeriveInput},
};

pub(crate) use crate::attr::MethodOrFunction;

macro_rules! dbg {
    ($val:expr $(,)?) => {
        // Use of `match` here is intentional because it affects the lifetimes
        // of temporaries - https://stackoverflow.com/a/48732525/1063961
        match $val {
            tmp => {
                #[cfg(feature = "debug")]
                eprintln!(
                    "[{}:{}:{}] {} = {}",
                    std::file!(),
                    std::line!(),
                    std::column!(),
                    std::stringify!($val),
                    &tmp
                );
                tmp
            },
        }
    };
}

/// # mlua_gen
/// Allow quick modifications of Rust's struct and enum in Lua with `mlua`
///
/// ## Struct
/// ### Example
/// The following rust code
/// ```rust
/// use mlua_gen::mlua_gen;
///
/// #[mlua_gen(impl = [default(), age(&self), set_age(&mut self, u8)])]
/// #[derive(Default)]
/// pub(crate) struct Human {
///     pub(crate) name: String,
///     age: u8,
/// }
///
/// impl Human {
///     pub(crate) fn age(&self) -> String {
///         format!("{} years old", self.age)
///     }
///     pub(crate) fn set_age(&mut self, age: u8) {
///         self.age = age;
///     }
/// }
/// ```
///
/// Can be used in lua easily like this
/// ```lua
/// local human = Human.default()
/// human:set_age(42)
/// human.name = "Martin"
/// assert(human.name == "Martin")
/// assert(human:age() == "42 years old")
/// ```
///
/// ### Attributes
/// #### `get`
/// Value: `pub` | `pub(crate)` | `pub(super)` | `*` | `[Ident]`
///
/// Define which attributes should have getters values. (By default `pub(super)`)
///
/// - `pub(super)` includes `pub` and `pub(crate)`
/// - `pub(crate)` includes `pub`
/// - `*` represents all fields
/// - `[Ident]` which fields should have getter ?
///
/// ##### Example
/// ```rust,ignore
/// #[mlua_gen(get = [a, c])]
/// struct MyFields {
///     a: usize,
///     pub(super) b: usize,
///     pub(crate) c: usize,
///     pub d: usize,
/// }
/// ```
/// Lua:
/// ```lua
/// print(fields.a)
/// print(fields.b) -- nil
/// print(fields.c)
/// print(fields.d) -- nil
/// ```
///
/// #### `set`
/// Value: `pub` | `pub(crate)` | `pub(super)` | `*` | `[Ident]`
///
/// See `get` for more information about visibility
///
/// Define which attributes should have setters values. (By default `pub(super)`)
///
/// ##### Example
/// ```rust,ignore
/// #[mlua_gen(set = [a, c])]
/// struct MyFields {
///     a: usize,
///     pub(super) b: usize,
///     pub(crate) c: usize,
///     pub d: usize,
/// }
/// ```
/// Lua:
/// ```lua
/// fields.a = 42
/// -- fields.b cannot be set
/// fields.c = 42
/// -- fields.d cannot be set
/// ```
///
/// #### `impl`
/// Value: `[ExprCall]` (`ExprCall` being a function call expression: `invoke(a, b)`)
///
/// The functions that are implemented for this struct and that you want to be usable in Lua
///
/// ##### Example
/// ```rust,ignore
/// #[mlua_gen(impl = [default(), age(&self), set_age(&mut self, u8)])]
/// #[derive(Default)]
/// pub(crate) struct Human {
///     pub(crate) name: String,
///     age: u8,
/// }
///
/// impl Human {
///     pub(crate) fn age(&self) -> String {
///         format!("{} years old", self.age)
///     }
///     pub(crate) fn set_age(&mut self, age: u8) {
///         self.age = age;
///     }
/// }
/// ```
///
/// Lua:
/// ```lua
/// local human = Human.default()
/// human:set_age(42)
/// print(human:age()) -- 42
/// ```
///
/// #### `custom_field`
/// Value: `Ident`
///
/// The `Ident` should be the ident of a function that has for signature
/// `fn(&mut ::mlua::UserDataFields<Self>) -> ();`
///
/// Posibility to add custom fields that will be usable in lua.
///
/// ##### Example
/// ```rust,ignore
/// #[mlua_gen(custom_fields = fields)]
/// struct Human {
///     pub name: String,
///     pub age: u8,
/// }
/// fn fields<T: ::mlua::UserDataFields<Human>>(fields: &mut T) {
///     fields.add_field_method_get("name_age", |_, this| {
///         Ok(format!("{} ({})", this.name, this.age))
///     });
/// }
/// ```
/// Lua:
/// ```lua
/// human.age = 42
/// human.name = "Martin"
/// assert(human.name_age == "Martin (42)")
/// ```
///
/// #### `custom_impls`
/// Value: `Ident`
///
/// Same as `custom_field` but with `UserDataMethods`
///
/// ## Enum
/// ### Example
/// The following rust code
/// ```rust
/// use mlua_gen::mlua_gen;
///
/// #[mlua_gen(impl = [default(), name(&self)])]
/// #[derive(Default)]
/// enum Animal {
///     #[default]
///     Pig,
///     Dog(String),
///     Cat { name: String, age: u8 },
/// }
///
/// impl Animal {
///     pub(crate) fn name(&self) -> String {
///         match self {
///             Animal::Pig => "Piggy".to_owned(),
///             Animal::Dog(name) => name.to_owned(),
///             Animal::Cat { name, .. } => name.to_owned(),
///         }
///     }
/// }
/// ```
///
/// Can be used easily with the following lua code
/// ```lua
/// local pig = Animal.Pig -- or in our case, Animal:default()
/// local dog = Animal.Dog ( "Doggo" )
/// local cat = Animal.Cat { name = "Neko", age = 8 }
///
/// -- method call
/// assert(pig:name() == "Piggy")
/// assert(dog:name() == "Doggo")
/// assert(cat:name() == "Neko")
///
/// -- access with field
/// --- Pig
/// assert(pig.pig)
/// assert(pig.dog == nil)
/// assert(pig.cat == nil)
/// --- Dog
/// assert(dog.pig == nil)
/// assert(dog.dog[1] == "Doggo")
/// assert(dog.cat == nil)
/// --- Cat
/// assert(cat.pig == nil)
/// assert(cat.dog == nil)
/// assert(cat.cat.name == "Neko")
/// assert(cat.cat.age == 8)
/// ```
///
#[proc_macro_attribute]
pub fn mlua_gen(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let generics = &input.generics;

    let mut attributes = Attributes::default();
    let attr_parser = syn::meta::parser(|meta| attributes.parse(meta));
    parse_macro_input!(args with attr_parser);

    let code = match input.data {
        Data::Struct(ref ds) => {
            match (|| -> syn::Result<TokenStream2> {
                let field_get = attributes.get.fields_from_visibility(&ds.fields)?;
                let field_set = attributes.set.fields_from_visibility(&ds.fields)?;
                Ok(struct_builder(
                    name,
                    generics,
                    &ds.fields,
                    field_get,
                    field_set,
                    attributes.custom_fields,
                    attributes.r#impl,
                    attributes.custom_impls,
                ))
            })() {
                Ok(e) => dbg!(e),
                Err(synerr) => return synerr.into_compile_error().into(),
            }
        },
        Data::Enum(ref de) => {
            dbg!(enum_builder(
                name,
                de.variants.iter(),
                attributes.custom_fields,
                attributes.custom_impls,
            ))
        },
        _ => panic!("Must annotate struct"),
    };

    quote! {
        #input

        #code
    }
    .into()
}
