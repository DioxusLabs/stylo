/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

<%! from data import to_rust_ident, to_camel_case, SYSTEM_FONT_LONGHANDS %>

<%def name="longhand(property)">
/// ${property.spec}
pub mod ${property.ident} {
    #[allow(unused_imports)]
    use crate::properties::longhand_prelude::*;

    #[allow(unused_variables)]
    pub unsafe fn cascade_property(
        declaration: &PropertyDeclaration,
        context: &mut computed::Context,
    ) {
        % if property.logical:
        declaration.debug_crash("Should physicalize before entering here");
        % else:
        % if property.is_zoom_dependent():
        unsafe fn reinherit_with_zoom(context: &mut computed::Context) {
            let computed = context.builder.inherited_style.clone_${property.ident}();
            let specified = computed::ToComputedValue::from_computed_value(&computed);
            % if property.boxed:
            let specified = Box::new(specified);
            % endif
            let decl = PropertyDeclaration::${property.camel_case}(specified);
            cascade_property(&decl, context);
        }
        % endif
        if declaration.cascade_simple_wide_keyword(
            context,
            LonghandId::${property.camel_case},
            /* is_inherited = */ ${"true" if property.style_struct.inherited else "false"},
            % if property.style_struct.inherited:
            |builder| builder.reset_${property.ident}(),
            % else:
            |builder| builder.inherit_${property.ident}(),
            % endif
            % if property.is_zoom_dependent():
            Some(reinherit_with_zoom),
            % else:
            None,
            % endif
        ) {
            return;
        }
        let specified_value = unsafe {
            declaration.unchecked_value_as::<${property.specified_type()}>()
        };

        % if property.ident in SYSTEM_FONT_LONGHANDS and engine == "gecko":
        if let Some(sf) = specified_value.get_system() {
            crate::properties::gecko::system_font::resolve_system_font(sf, context);
        }
        % endif

        % if property.vector and not property.vector.simple_bindings and engine == "gecko":
            // In the case of a vector property we want to pass down an
            // iterator so that this can be computed without allocation.
            //
            // However, computing requires a context, but the style struct
            // being mutated is on the context. We temporarily remove it,
            // mutate it, and then put it back. Vector longhands cannot
            // touch their own style struct whilst computing, else this will
            // panic.
            let mut s =
                context.builder.take_${property.style_struct.name_lower}();
            {
                let iter = specified_value.compute_iter(context);
                s.set_${property.ident}(iter);
            }
            context.builder.put_${property.style_struct.name_lower}(s);
        % else:
            % if property.boxed:
            let computed = (**specified_value).to_computed_value(context);
            % else:
            let computed = specified_value.to_computed_value(context);
            % endif
            context.builder.set_${property.ident}(computed)
        % endif
        % endif
    }

    pub fn parse_declared<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<PropertyDeclaration, ParseError<'i>> {
        parse(context, input)
        % if property.boxed:
            .map(Box::new)
        % endif
            .map(PropertyDeclaration::${property.camel_case})
    }

    % if property.vector:
    pub mod single_value {
        use super::*;
    % endif
    % if property.predefined_type:
    pub use crate::values::specified::${property.predefined_type} as SpecifiedValue;
    pub mod computed_value {
        pub use crate::values::computed::${property.predefined_type} as T;
    }
    % if property.initial_value:
    #[inline]
    pub fn get_initial_value() -> computed_value::T { ${property.initial_value} }
    #[inline]
    pub fn get_initial_specified_value() -> SpecifiedValue {
    % if property.initial_specified_value:
        ${property.initial_specified_value}
    % else:
        ToComputedValue::from_computed_value(&get_initial_value())
    % endif
    }
    % endif
    #[allow(unused_variables)]
    #[inline]
    pub fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<SpecifiedValue, ParseError<'i>> {
        % if property.allow_quirks:
        specified::${property.predefined_type}::${property.parse_method}_quirky(context, input, AllowQuirks::Yes)
        % elif property.parse_method != "parse":
        specified::${property.predefined_type}::${property.parse_method}(context, input)
        % else:
        <specified::${property.predefined_type} as crate::parser::Parse>::parse(context, input)
        % endif
    }
    % elif property.keyword:
    pub use self::computed_value::T as SpecifiedValue;
    pub mod computed_value {
        #[allow(unused_imports)]
        use crate::derives::*;
        #[derive(
            Clone,
            Copy,
            Debug,
            Deserialize,
            Eq,
            FromPrimitive,
            Hash,
            MallocSizeOf,
            Parse,
            PartialEq,
            Serialize,
            SpecifiedValueInfo,
            ToAnimatedValue,
            ToComputedValue,
            ToCss,
            ToResolvedValue,
            ToShmem,
            ToTyped,
        )]
        pub enum T {
        % for variant in property.keyword.values_for(engine):
        <%
            aliases = []
            for alias, v in property.keyword.aliases_for(engine).items():
                if variant == v:
                    aliases.append(alias)
        %>
        % if aliases:
        #[parse(aliases = "${','.join(sorted(aliases))}")]
        % endif
        ${to_camel_case(variant)},
        % endfor
        }
    }
    #[inline]
    pub fn get_initial_value() -> computed_value::T {
        computed_value::T::${to_camel_case(property.keyword.values[0])}
    }
    #[inline]
    pub fn get_initial_specified_value() -> SpecifiedValue {
        SpecifiedValue::${to_camel_case(property.keyword.values[0])}
    }
    #[inline]
    pub fn parse<'i, 't>(_context: &ParserContext, input: &mut Parser<'i, 't>)
                         -> Result<SpecifiedValue, ParseError<'i>> {
        SpecifiedValue::parse(input)
    }

    % if engine == "gecko":
    #[cfg(feature = "gecko")]
    impl SpecifiedValue {
        /// Obtain a specified value from a Gecko keyword value
        ///
        /// Intended for use with presentation attributes, not style structs
        pub fn from_gecko_keyword(kw: u32) -> Self {
            use crate::gecko_bindings::structs;
            % for value in property.keyword.values_for(engine):
            // We can't match on enum values if we're matching on a u32
            const ${to_rust_ident(value).upper()}: u32 = structs::${property.keyword.gecko_constant(value)} as u32;
            % endfor
            match kw {
                % for value in property.keyword.values_for(engine):
                ${to_rust_ident(value).upper()} => Self::${to_camel_case(value)},
                % endfor
                _ => panic!("Found unexpected value in style struct for ${property.name} property"),
            }
        }
    }
    % endif
    % endif
    % if property.vector:
    } // single_value
    % endif

    // The setup here is roughly:
    //
    //  * UnderlyingList is the list that is stored in the computed value. This may
    //    be a shared ArcSlice if the property is inherited.
    //  * UnderlyingOwnedList is the list that is used for animation.
    //  * Specified values always use OwnedSlice, since it's more compact.
    //  * computed_value::List is just a convenient alias that you can use for the
    //    computed value list, since this is in the computed_value module.
    //
    // If vector.simple_bindings is true, then we don't use the complex iterator
    // machinery and set_foo_from, and just compute the value like any other
    // longhand.
    % if property.vector:
    <%
        allow_empty = not property.initial_value and not property.keyword
        is_shared_list = allow_empty and property.style_struct.inherited
        if not allow_empty:
            list_type = "NonEmptyCommaList"
        elif property.vector.separator == "Comma":
            list_type = "EmptyCommaList"
        else:
            list_type = "EmptySpaceList"
    %>
    /// The definition of the computed value for ${property.name}.
    pub mod computed_value {
        #[allow(unused_imports)]
        use crate::values::animated::ToAnimatedValue;
        #[allow(unused_imports)]
        use crate::values::resolved::ToResolvedValue;
        #[allow(unused_imports)]
        use crate::derives::*;
        pub use super::single_value::computed_value as single_value;
        pub use self::single_value::T as SingleComputedValue;
        % if not allow_empty:
        use smallvec::SmallVec;
        % endif
        use crate::values::computed::ComputedVecIter;

        // FIXME(emilio): Add an OwnedNonEmptySlice type, and figure out
        // something for transition-name, which is the only remaining user
        // of NotInitial.
        pub type UnderlyingList<T> =
            % if allow_empty:
            % if property.style_struct.inherited:
                crate::ArcSlice<T>;
            % else:
                crate::OwnedSlice<T>;
            % endif
            % else:
                SmallVec<[T; 1]>;
            % endif

        pub type UnderlyingOwnedList<T> =
            % if allow_empty:
                crate::OwnedSlice<T>;
            % else:
                SmallVec<[T; 1]>;
            % endif

        /// The generic type defining the animated and resolved values for
        /// this property.
        pub use crate::properties::longhand_lists::${list_type} as OwnedList;

        /// The computed value for this property.
        % if not is_shared_list:
        pub type ComputedList = OwnedList<single_value::T>;
        pub use self::OwnedList as List;
        % else:
        pub use self::ComputedList as List;

        #[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToTyped)]
        % if property.vector.separator == "Comma":
        #[css(comma)]
        % endif
        pub struct ComputedList(
            % if not allow_empty:
            #[css(iterable)]
            % else:
            #[css(if_empty = "none", iterable)]
            % endif
            % if is_shared_list:
            #[ignore_malloc_size_of = "Arc"]
            % endif
            pub UnderlyingList<single_value::T>,
        );

        type ResolvedList = <OwnedList<single_value::T> as ToResolvedValue>::ResolvedValue;
        impl ToResolvedValue for ComputedList {
            type ResolvedValue = ResolvedList;

            fn to_resolved_value(self, context: &crate::values::resolved::Context) -> Self::ResolvedValue {
                OwnedList(
                    self.0
                        .iter()
                        .cloned()
                        .map(|v| v.to_resolved_value(context))
                        .collect()
                )
            }

            fn from_resolved_value(resolved: Self::ResolvedValue) -> Self {
                let iter =
                    resolved.0.into_iter().map(ToResolvedValue::from_resolved_value);
                ComputedList(UnderlyingList::from_iter(iter))
            }
        }

        % if property.vector.simple_bindings:
        impl From<ComputedList> for UnderlyingList<single_value::T> {
            #[inline]
            fn from(l: ComputedList) -> Self {
                l.0
            }
        }
        impl From<UnderlyingList<single_value::T>> for ComputedList {
            #[inline]
            fn from(l: UnderlyingList<single_value::T>) -> Self {
                List(l)
            }
        }
        % endif

        % if property.vector.animation_type:
        type AnimatedList = <OwnedList<single_value::T> as ToAnimatedValue>::AnimatedValue;
        impl ToAnimatedValue for ComputedList {
            type AnimatedValue = AnimatedList;

            fn to_animated_value(self, context: &crate::values::animated::Context) -> Self::AnimatedValue {
                OwnedList(
                    self.0.iter().map(|v| v.clone().to_animated_value(context)).collect()
                )
            }

            fn from_animated_value(animated: Self::AnimatedValue) -> Self {
                let iter =
                    animated.0.into_iter().map(ToAnimatedValue::from_animated_value);
                ComputedList(UnderlyingList::from_iter(iter))
            }
        }
        % endif
        % endif

        /// The computed value, effectively a list of single values.
        pub use self::ComputedList as T;

        pub type Iter<'a, 'cx, 'cx_a> = ComputedVecIter<'a, 'cx, 'cx_a, super::single_value::SpecifiedValue>;
    }

    /// The specified value of ${property.name}.
    #[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
    % if property.vector.none_value:
    #[value_info(other_values = "none")]
    % endif
    % if property.vector.separator == "Comma":
    #[css(comma)]
    % endif
    pub struct SpecifiedValue(
        % if not allow_empty:
        #[css(iterable)]
        % else:
        #[css(if_empty = "none", iterable)]
        % endif
        pub crate::OwnedSlice<single_value::SpecifiedValue>,
    );

    pub fn get_initial_value() -> computed_value::T {
        % if allow_empty:
            computed_value::List(Default::default())
        % else:
            let mut v = SmallVec::new();
            v.push(single_value::get_initial_value());
            computed_value::List(v)
        % endif
    }

    pub fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<SpecifiedValue, ParseError<'i>> {
        use style_traits::Separator;

        % if allow_empty or property.vector.none_value:
        if input.try_parse(|input| input.expect_ident_matching("none")).is_ok() {
            % if allow_empty:
            return Ok(SpecifiedValue(Default::default()))
            % else:
            return Ok(SpecifiedValue(crate::OwnedSlice::from(vec![${property.vector.none_value}])))
            % endif
        }
        % endif

        let v = style_traits::${property.vector.separator}::parse(input, |parser| {
            single_value::parse(context, parser)
        })?;
        Ok(SpecifiedValue(v.into()))
    }

    pub use self::single_value::SpecifiedValue as SingleSpecifiedValue;

    % if not property.vector.simple_bindings and engine == "gecko":
    impl SpecifiedValue {
        fn compute_iter<'a, 'cx, 'cx_a>(
            &'a self,
            context: &'cx computed::Context<'cx_a>,
        ) -> computed_value::Iter<'a, 'cx, 'cx_a> {
            computed_value::Iter::new(context, &self.0)
        }
    }
    % endif

    impl ToComputedValue for SpecifiedValue {
        type ComputedValue = computed_value::T;

        #[inline]
        fn to_computed_value(&self, context: &computed::Context) -> computed_value::T {
            % if not is_shared_list:
            use std::iter::FromIterator;
            % endif
            computed_value::List(computed_value::UnderlyingList::from_iter(
                self.0.iter().map(|i| i.to_computed_value(context))
            ))
        }

        #[inline]
        fn from_computed_value(computed: &computed_value::T) -> Self {
            let iter = computed.0.iter().map(ToComputedValue::from_computed_value);
            SpecifiedValue(iter.collect())
        }
    }
    % endif
}
</%def>

<%def name="shorthand(shorthand)">
    /// ${shorthand.spec}
    pub mod ${shorthand.ident} {
        #[allow(unused_imports)]
        use crate::properties::shorthand_prelude::*;

        % if shorthand.derive_value_info:
        #[derive(SpecifiedValueInfo)]
        % endif
        pub struct Longhands {
            % for sub_property in shorthand.sub_properties:
                pub ${sub_property.ident}:
                    % if sub_property.boxed:
                        Box<
                    % endif
                    longhands::${sub_property.ident}::SpecifiedValue
                    % if sub_property.boxed:
                        >
                    % endif
                    ,
            % endfor
        }

        /// Represents a serializable set of all of the longhand properties that
        /// correspond to a shorthand.
        % if shorthand.derive_serialize:
        #[derive(ToCss)]
        % endif
        pub struct LonghandsToSerialize<'a> {
            % for sub_property in shorthand.sub_properties:
                pub ${sub_property.ident}:
                % if sub_property.may_be_disabled_in(shorthand, engine):
                    Option<
                % endif
                    &'a longhands::${sub_property.ident}::SpecifiedValue,
                % if sub_property.may_be_disabled_in(shorthand, engine):
                    >,
                % endif
            % endfor
        }

        impl<'a> LonghandsToSerialize<'a> {
            /// Tries to get a serializable set of longhands given a set of
            /// property declarations.
            pub fn from_iter(iter: impl Iterator<Item = &'a PropertyDeclaration>) -> Result<Self, ()> {
                // Define all of the expected variables that correspond to the shorthand
                % for sub_property in shorthand.sub_properties:
                    let mut ${sub_property.ident} =
                        None::<&'a longhands::${sub_property.ident}::SpecifiedValue>;
                % endfor

                // Attempt to assign the incoming declarations to the expected variables
                for declaration in iter {
                    match *declaration {
                        % for sub_property in shorthand.sub_properties:
                            PropertyDeclaration::${sub_property.camel_case}(ref value) => {
                                ${sub_property.ident} = Some(value)
                            },
                        % endfor
                        _ => {}
                    };
                }

                // If any of the expected variables are missing, return an error
                match (
                    % for sub_property in shorthand.sub_properties:
                        ${sub_property.ident},
                    % endfor
                ) {

                    (
                    % for sub_property in shorthand.sub_properties:
                        % if sub_property.may_be_disabled_in(shorthand, engine):
                        ${sub_property.ident},
                        % else:
                        Some(${sub_property.ident}),
                        % endif
                    % endfor
                    ) =>
                    Ok(LonghandsToSerialize {
                        % for sub_property in shorthand.sub_properties:
                            ${sub_property.ident},
                        % endfor
                    }),
                    _ => Err(())
                }
            }
        }

        /// Parse the given shorthand and fill the result into the
        /// `declarations` vector.
        pub fn parse_into<'i, 't>(
            declarations: &mut SourcePropertyDeclaration,
            context: &ParserContext,
            input: &mut Parser<'i, 't>,
        ) -> Result<(), ParseError<'i>> {
            #[allow(unused_imports)]
            use crate::properties::{NonCustomPropertyId, LonghandId};
            % if not shorthand.kind:
            use crate::properties::shorthands::${shorthand.ident}::parse_value;
            % endif
            input.parse_entirely(|input| parse_value(context, input)).map(|longhands| {
                % for sub_property in shorthand.sub_properties:
                % if sub_property.may_be_disabled_in(shorthand, engine):
                if NonCustomPropertyId::from(LonghandId::${sub_property.camel_case})
                    .allowed_in_ignoring_rule_type(context) {
                % endif
                    declarations.push(PropertyDeclaration::${sub_property.camel_case}(
                        longhands.${sub_property.ident}
                    ));
                % if sub_property.may_be_disabled_in(shorthand, engine):
                }
                % endif
                % endfor
            })
        }

        /// Try to serialize a given shorthand to a string.
        pub fn to_css(declarations: &[&PropertyDeclaration], dest: &mut style_traits::CssStringWriter) -> fmt::Result {
            match LonghandsToSerialize::from_iter(declarations.iter().cloned()) {
                Ok(longhands) => longhands.to_css(&mut CssWriter::new(dest)),
                Err(_) => Ok(())
            }
        }
        % if shorthand.kind == "two_properties":
        ${self.two_properties_shorthand(shorthand)}
        % endif
        % if shorthand.kind == "four_sides":
        ${self.four_sides_shorthand(shorthand)}
        % endif
        % if shorthand.kind == "single_border":
        ${self.single_border_shorthand(shorthand)}
        % endif
    }
</%def>

// A shorthand of kind `<property-1> <property-2>?` where both properties have
// the same type.
<%def name="two_properties_shorthand(shorthand)">
    type Single = crate::properties::longhands::${shorthand.sub_properties[0].ident}::SpecifiedValue;

    fn parse_value<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Longhands, ParseError<'i>> {
        let first = <Single as crate::parser::Parse>::parse(context, input)?;
        let second =
            input.try_parse(|input| <Single as crate::parser::Parse>::parse(context, input)).unwrap_or_else(|_| first.clone());
        Ok(crate::properties::shorthands::expanded! {
            ${shorthand.sub_properties[0].ident}: first,
            ${shorthand.sub_properties[1].ident}: second,
        })
    }

    impl<'a> ToCss for LonghandsToSerialize<'a>  {
        fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result where W: fmt::Write {
            let first = &self.${shorthand.sub_properties[0].ident};
            let second = &self.${shorthand.sub_properties[1].ident};

            first.to_css(dest)?;
            if first != second {
                dest.write_char(' ')?;
                second.to_css(dest)?;
            }
            Ok(())
        }
    }
</%def>

<%def name="four_sides_shorthand(shorthand)">
    use crate::values::generics::rect::Rect;

    type Single = crate::properties::longhands::${shorthand.sub_properties[0].ident}::SpecifiedValue;
    fn parse_value<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Longhands, ParseError<'i>> {
        let rect = Rect::parse_with(context, input, |c, i| -> Result<Single, ParseError<'i> > {
        % if shorthand.allow_quirks:
            Single::parse_quirky(c, i, crate::values::specified::AllowQuirks::Yes)
        % else:
            <Single as crate::parser::Parse>::parse(c, i)
        % endif
        })?;
        Ok(crate::properties::shorthands::expanded! {
        % for index in range(4):
            ${shorthand.sub_properties[index].ident}: rect.${index},
        % endfor
        })
    }

    impl<'a> ToCss for LonghandsToSerialize<'a> {
        fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
        where
            W: fmt::Write,
        {
            let rect = Rect::new(
                % for index in range(4):
                    &self.${shorthand.sub_properties[index].ident},
                % endfor
            );
            rect.to_css(dest)
        }
    }
</%def>

<%def name="single_border_shorthand(shorthand)">
    fn parse_value<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Longhands, ParseError<'i>> {
        let (width, style, color) = crate::properties::shorthands::parse_border(context, input)?;
        Ok(crate::properties::shorthands::expanded! {
            ${shorthand.sub_properties[0].ident}: width,
            ${shorthand.sub_properties[1].ident}: style,
            ${shorthand.sub_properties[2].ident}: color,
        })
    }

    impl<'a> ToCss for LonghandsToSerialize<'a>  {
        fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result where W: fmt::Write {
            crate::properties::shorthands::serialize_directional_border(
                dest,
                % for i in range(3):
                self.${shorthand.sub_properties[i].ident},
                % endfor
            )
        }
    }
</%def>
