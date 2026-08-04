<%! from data import SYSTEM_FONT_LONGHANDS %>
    //! We deal with system fonts here
    //!
    //! System fonts can only be set as a group via the font shorthand.
    //! They resolve at compute time (not parse time -- this lets the
    //! browser respond to changes to the OS font settings).
    //!
    //! While Gecko handles these as a separate property and keyword
    //! values on each property indicating that the font should be picked
    //! from the -x-system-font property, we avoid this. Instead,
    //! each font longhand has a special SystemFont variant which contains
    //! the specified system font. When the cascade function (in helpers)
    //! detects that a value has a system font, it will resolve it, and
    //! cache it on the ComputedValues. After this, it can be just fetched
    //! whenever a font longhand on the same element needs the system font.
    //!
    //! When a longhand property is holding a SystemFont, it's serialized
    //! to an empty string as if its value comes from a shorthand with
    //! variable reference. We may want to improve this behavior at some
    //! point. See also https://github.com/w3c/csswg-drafts/issues/1586.

    use crate::properties::longhands;
    use std::hash::{Hash, Hasher};
    use crate::values::computed::{ToComputedValue, Context};
    use crate::values::specified::font::SystemFont;
    // ComputedValues are compared at times
    // so we need these impls. We don't want to
    // add Eq to Number (which contains a float)
    // so instead we have an eq impl which skips the
    // cached values
    impl PartialEq for ComputedSystemFont {
        fn eq(&self, other: &Self) -> bool {
            self.system_font == other.system_font
        }
    }
    impl Eq for ComputedSystemFont {}

    impl Hash for ComputedSystemFont {
        fn hash<H: Hasher>(&self, hasher: &mut H) {
            self.system_font.hash(hasher)
        }
    }

    impl ToComputedValue for SystemFont {
        type ComputedValue = ComputedSystemFont;

        fn to_computed_value(&self, cx: &Context) -> Self::ComputedValue {
            use crate::gecko_bindings::bindings;
            use crate::gecko_bindings::structs::nsFont;
            use crate::values::computed::font::FontSize;
            use crate::values::specified::font::KeywordInfo;
            use crate::values::generics::NonNegative;
            use std::mem;

            let mut system = mem::MaybeUninit::<nsFont>::uninit();
            let system = unsafe {
                bindings::Gecko_nsFont_InitSystem(
                    system.as_mut_ptr(),
                    *self,
                    &**cx.style().get_font(),
                    cx.device().document()
                );
                &mut *system.as_mut_ptr()
            };
            let size = NonNegative(cx.maybe_zoom_text(system.size.0));
            let ret = ComputedSystemFont {
                font_family: system.family.clone(),
                font_size: FontSize {
                    computed_size: size,
                    used_size: size,
                    keyword_info: KeywordInfo::none()
                },
                font_weight: system.weight,
                font_stretch: system.stretch,
                font_style: system.style,
                system_font: *self,
            };
            unsafe { bindings::Gecko_nsFont_Destroy(system); }
            ret
        }

        fn from_computed_value(_: &ComputedSystemFont) -> Self {
            unreachable!()
        }
    }

    #[inline]
    /// Compute and cache a system font
    ///
    /// Must be called before attempting to compute a system font
    /// specified value
    pub fn resolve_system_font(system: SystemFont, context: &mut Context) {
        // Checking if context.cached_system_font.is_none() isn't enough,
        // if animating from one system font to another the cached system font
        // may change
        if context.cached_system_font.as_ref().is_none_or(|x| x.system_font != system) {
            let computed = system.to_computed_value(context);
            context.cached_system_font = Some(computed);
        }
    }

    #[derive(Clone, Debug)]
    pub struct ComputedSystemFont {
        % for name in SYSTEM_FONT_LONGHANDS:
            pub ${name}: longhands::${name}::computed_value::T,
        % endfor
        pub system_font: SystemFont,
    }
