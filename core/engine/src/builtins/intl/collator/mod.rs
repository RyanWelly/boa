use boa_gc::{custom_trace, Finalize, Trace};
use boa_profiler::Profiler;
use icu_collator::{
    options::{AlternateHandling, MaxVariable},
    preferences::{CollationCaseFirst, CollationNumericOrdering, CollationType},
    provider::CollationMetadataV1,
    CollatorPreferences,
};

use icu_locale::{
    extensions::unicode::Value, extensions_unicode_key as key, extensions_unicode_value as value,
    Locale,
};
use icu_provider::DataMarkerAttributes;

use crate::{
    builtins::{
        options::get_option, BuiltInBuilder, BuiltInConstructor, BuiltInObject, IntrinsicObject,
        OrdinaryObject,
    },
    context::{
        icu::IntlProvider,
        intrinsics::{Intrinsics, StandardConstructor, StandardConstructors},
    },
    js_string,
    native_function::NativeFunction,
    object::{
        internal_methods::get_prototype_from_constructor, FunctionObjectBuilder, JsFunction,
        JsObject,
    },
    property::Attribute,
    realm::Realm,
    string::StaticJsStrings,
    symbol::JsSymbol,
    Context, JsArgs, JsData, JsNativeError, JsResult, JsString, JsValue,
};

use super::{
    locale::{canonicalize_locale_list, filter_locales, resolve_locale, validate_extension},
    options::{coerce_options_to_object, IntlOptions},
    Service,
};

mod options;
pub(crate) use options::*;

#[derive(Debug, Finalize, JsData)]
#[allow(clippy::struct_field_names)]
pub(crate) struct Collator {
    locale: Locale,
    collation: Value,
    numeric: bool,
    case_first: Option<CollationCaseFirst>,
    usage: Usage,
    sensitivity: Sensitivity,
    ignore_punctuation: bool,
    collator: icu_collator::Collator,
    bound_compare: Option<JsFunction>,
}

// SAFETY: only `bound_compare` is a traceable object.
unsafe impl Trace for Collator {
    custom_trace!(this, mark, mark(&this.bound_compare));
}

impl Collator {
    /// Gets the inner [`icu_collator::Collator`] comparator.
    pub(crate) const fn collator(&self) -> &icu_collator::Collator {
        &self.collator
    }
}

#[derive(Debug, Clone)]
pub(super) struct CollatorLocaleOptions {
    collation: Option<Value>,
    numeric: Option<bool>,
    case_first: Option<CollationCaseFirst>,
}

impl Service for Collator {
    type LangMarker = CollationMetadataV1;

    type LocaleOptions = CollatorLocaleOptions;

    fn resolve(locale: &mut Locale, options: &mut Self::LocaleOptions, provider: &IntlProvider) {
        let collation = options
            .collation
            .take()
            .filter(|co| {
                CollationType::try_from(co).is_ok_and(|co| {
                    let attr = DataMarkerAttributes::from_str_or_panic(co.as_str());
                    validate_extension::<Self::LangMarker>(locale.id.clone(), attr, provider)
                })
            })
            .or_else(|| {
                locale
                    .extensions
                    .unicode
                    .keywords
                    .get(&key!("co"))
                    .cloned()
                    .filter(|co| {
                        CollationType::try_from(co).is_ok_and(|co| {
                            let attr = DataMarkerAttributes::from_str_or_panic(co.as_str());
                            validate_extension::<Self::LangMarker>(
                                locale.id.clone(),
                                attr,
                                provider,
                            )
                        })
                    })
            })
            .filter(|co| co != &value!("search"));

        let numeric =
            options.numeric.or_else(
                || match locale.extensions.unicode.keywords.get(&key!("kn")) {
                    Some(a) if a == &value!("true") => Some(true),
                    Some(_) => Some(false),
                    _ => None,
                },
            );

        let case_first = options.case_first.or_else(|| {
            match locale.extensions.unicode.keywords.get(&key!("kf")) {
                Some(a) if a == &value!("upper") => Some(CollationCaseFirst::Upper),
                Some(a) if a == &value!("lower") => Some(CollationCaseFirst::Lower),
                Some(_) => Some(CollationCaseFirst::False),
                _ => None,
            }
        });

        locale.extensions.unicode.clear();

        if let Some(co) = collation.clone() {
            locale.extensions.unicode.keywords.set(key!("co"), co);
        }

        if let Some(kn) = numeric.map(|kn| if kn { value!("true") } else { value!("false") }) {
            locale.extensions.unicode.keywords.set(key!("kn"), kn);
        }

        if let Some(kf) = case_first.map(|kf| match kf {
            CollationCaseFirst::False => value!("false"),
            CollationCaseFirst::Lower => value!("lower"),
            CollationCaseFirst::Upper => value!("upper"),
            _ => unreachable!(),
        }) {
            locale.extensions.unicode.keywords.set(key!("kf"), kf);
        }

        options.collation = collation;
        options.numeric = numeric;
        options.case_first = case_first;
    }
}

impl IntrinsicObject for Collator {
    fn init(realm: &Realm) {
        let _timer = Profiler::global().start_event(std::any::type_name::<Self>(), "init");

        let compare = BuiltInBuilder::callable(realm, Self::compare)
            .name(js_string!("get compare"))
            .build();

        BuiltInBuilder::from_standard_constructor::<Self>(realm)
            .static_method(
                Self::supported_locales_of,
                js_string!("supportedLocalesOf"),
                1,
            )
            .property(
                JsSymbol::to_string_tag(),
                js_string!("Intl.Collator"),
                Attribute::CONFIGURABLE,
            )
            .accessor(
                js_string!("compare"),
                Some(compare),
                None,
                Attribute::CONFIGURABLE,
            )
            .method(Self::resolved_options, js_string!("resolvedOptions"), 0)
            .build();
    }

    fn get(intrinsics: &Intrinsics) -> JsObject {
        Self::STANDARD_CONSTRUCTOR(intrinsics.constructors()).constructor()
    }
}

impl BuiltInObject for Collator {
    const NAME: JsString = StaticJsStrings::COLLATOR;
}

impl BuiltInConstructor for Collator {
    const LENGTH: usize = 0;
    const P: usize = 3;
    const SP: usize = 1;

    const STANDARD_CONSTRUCTOR: fn(&StandardConstructors) -> &StandardConstructor =
        StandardConstructors::collator;

    /// Constructor [`Intl.Collator ( [ locales [ , options ] ] )`][spec].
    ///
    /// Constructor for `Collator` objects.
    ///
    /// More information:
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma402/#sec-intl.collator
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Intl/Collator
    fn constructor(
        new_target: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        // 1. If NewTarget is undefined, let newTarget be the active function object, else let newTarget be NewTarget.
        let new_target = &if new_target.is_undefined() {
            context
                .active_function_object()
                .unwrap_or_else(|| context.intrinsics().constructors().collator().constructor())
                .into()
        } else {
            new_target.clone()
        };
        // 2. Let internalSlotsList be « [[InitializedCollator]], [[Locale]], [[Usage]], [[Sensitivity]], [[IgnorePunctuation]], [[Collation]], [[BoundCompare]] ».
        // 3. If %Collator%.[[RelevantExtensionKeys]] contains "kn", then
        //     a. Append [[Numeric]] as the last element of internalSlotsList.
        // 4. If %Collator%.[[RelevantExtensionKeys]] contains "kf", then
        //     a. Append [[CaseFirst]] as the last element of internalSlotsList.
        // 5. Let collator be ? OrdinaryCreateFromConstructor(newTarget, "%Collator.prototype%", internalSlotsList).
        // 6. Return ? InitializeCollator(collator, locales, options).

        let locales = args.get_or_undefined(0);
        let options = args.get_or_undefined(1);

        // Abstract operation `InitializeCollator ( collator, locales, options )`
        // https://tc39.es/ecma402/#sec-initializecollator

        // 1. Let requestedLocales be ? CanonicalizeLocaleList(locales).
        let requested_locales = canonicalize_locale_list(locales, context)?;

        // 2. Set options to ? CoerceOptionsToObject(options).
        let options = coerce_options_to_object(options, context)?;

        // 3. Let usage be ? GetOption(options, "usage", string, « "sort", "search" », "sort").
        // 4. Set collator.[[Usage]] to usage.
        // 5. If usage is "sort", then
        //     a. Let localeData be %Collator%.[[SortLocaleData]].
        // 6. Else,
        //     a. Let localeData be %Collator%.[[SearchLocaleData]].
        let usage = get_option(&options, js_string!("usage"), context)?.unwrap_or_default();

        // 7. Let opt be a new Record.
        // 8. Let matcher be ? GetOption(options, "localeMatcher", string, « "lookup", "best fit" », "best fit").
        // 9. Set opt.[[localeMatcher]] to matcher.
        let matcher =
            get_option(&options, js_string!("localeMatcher"), context)?.unwrap_or_default();

        // 10. Let collation be ? GetOption(options, "collation", string, empty, undefined).
        // 11. If collation is not undefined, then
        //     a. If collation does not match the Unicode Locale Identifier type nonterminal, throw a RangeError exception.
        // 12. Set opt.[[co]] to collation.
        let collation = get_option(&options, js_string!("collation"), context)?;

        // 13. Let numeric be ? GetOption(options, "numeric", boolean, empty, undefined).
        // 14. If numeric is not undefined, then
        //     a. Let numeric be ! ToString(numeric).
        // 15. Set opt.[[kn]] to numeric.
        let numeric = get_option(&options, js_string!("numeric"), context)?;

        // 16. Let caseFirst be ? GetOption(options, "caseFirst", string, « "upper", "lower", "false" », undefined).
        // 17. Set opt.[[kf]] to caseFirst.
        let case_first = get_option(&options, js_string!("caseFirst"), context)?;

        let mut intl_options = IntlOptions {
            matcher,
            service_options: CollatorLocaleOptions {
                collation,
                numeric,
                case_first,
            },
        };

        // 18. Let relevantExtensionKeys be %Collator%.[[RelevantExtensionKeys]].
        // 19. Let r be ResolveLocale(%Collator%.[[AvailableLocales]], requestedLocales, opt, relevantExtensionKeys, localeData).
        let locale = resolve_locale::<Self>(
            requested_locales,
            &mut intl_options,
            context.intl_provider(),
        )?;

        let mut locale_prefs = CollatorPreferences::from(&locale);

        // 20. Set collator.[[Locale]] to r.[[locale]].

        // 21. Let collation be r.[[co]].
        // 22. If collation is null, let collation be "default".
        // 23. Set collator.[[Collation]] to collation.
        let collation = intl_options
            .service_options
            .collation
            .unwrap_or(value!("default"));

        // 24. If relevantExtensionKeys contains "kn", then
        //     a. Set collator.[[Numeric]] to SameValue(r.[[kn]], "true").
        let numeric = intl_options.service_options.numeric.unwrap_or_default();

        // 25. If relevantExtensionKeys contains "kf", then
        //     a. Set collator.[[CaseFirst]] to r.[[kf]].
        let case_first = intl_options.service_options.case_first;

        // 26. Let sensitivity be ? GetOption(options, "sensitivity", string, « "base", "accent", "case", "variant" », undefined).
        // 28. Set collator.[[Sensitivity]] to sensitivity.
        let sensitivity = get_option(&options, js_string!("sensitivity"), context)?
            // 27. If sensitivity is undefined, then
            //     a. If usage is "sort", then
            //         i. Let sensitivity be "variant".
            //     b. Else,
            //         i. Let dataLocale be r.[[dataLocale]].
            //         ii. Let dataLocaleData be localeData.[[<dataLocale>]].
            //         iii. Let sensitivity be dataLocaleData.[[sensitivity]].
            .or_else(|| (usage == Usage::Sort).then_some(Sensitivity::Variant));

        // 29. Let ignorePunctuation be ? GetOption(options, "ignorePunctuation", boolean, empty, false).
        // 30. Set collator.[[IgnorePunctuation]] to ignorePunctuation.
        let ignore_punctuation: bool =
            get_option(&options, js_string!("ignorePunctuation"), context)?.unwrap_or_default();

        let (strength, case_level) = sensitivity.map(Sensitivity::to_collator_options).unzip();

        let (alternate_handling, max_variable) = ignore_punctuation
            .then_some((AlternateHandling::Shifted, MaxVariable::Punctuation))
            .unzip();

        let mut options = icu_collator::options::CollatorOptions::default();
        options.strength = strength;
        options.case_level = case_level;
        options.alternate_handling = alternate_handling;
        options.max_variable = max_variable;

        let mut prefs = CollatorPreferences::default();
        prefs.case_first = case_first;
        prefs.numeric_ordering = Some(if numeric {
            CollationNumericOrdering::True
        } else {
            CollationNumericOrdering::False
        });
        if usage == Usage::Search {
            prefs.collation_type = Some(CollationType::Search);
        }
        locale_prefs.extend(prefs);

        let collator = icu_collator::Collator::try_new_with_buffer_provider(
            context.intl_provider().erased_provider(),
            locale_prefs,
            options,
        )
        .map_err(|e| JsNativeError::typ().with_message(e.to_string()))?;

        let prototype =
            get_prototype_from_constructor(new_target, StandardConstructors::collator, context)?;
        let collator = JsObject::from_proto_and_data_with_shared_shape(
            context.root_shape(),
            prototype,
            Self {
                locale,
                collation,
                numeric,
                case_first,
                usage,
                sensitivity: sensitivity.unwrap_or(Sensitivity::Variant),
                ignore_punctuation,
                collator,
                bound_compare: None,
            },
        );

        // 31. Return collator.
        Ok(collator.into())
    }
}

impl Collator {
    /// [`Intl.Collator.supportedLocalesOf ( locales [ , options ] )`][spec].
    ///
    /// Returns an array containing those of the provided locales that are supported in collation
    /// without having to fall back to the runtime's default locale.
    ///
    /// More information:
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma402/#sec-intl.collator.supportedlocalesof
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Intl/Collator/supportedLocalesOf
    fn supported_locales_of(
        _: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        let locales = args.get_or_undefined(0);
        let options = args.get_or_undefined(1);

        // 1. Let availableLocales be %Collator%.[[AvailableLocales]].
        // 2. Let requestedLocales be ? CanonicalizeLocaleList(locales).
        let requested_locales = canonicalize_locale_list(locales, context)?;

        // 3. Return ? FilterLocales(availableLocales, requestedLocales, options).
        filter_locales::<Self>(requested_locales, options, context).map(JsValue::from)
    }

    /// [`get Intl.Collator.prototype.compare`][spec].
    ///
    /// Compares two strings according to the sort order of this Intl.Collator object.
    ///
    /// More information:
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma402/#sec-intl.collator.prototype.compare
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Intl/Collator/compare
    fn compare(this: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
        // 1. Let collator be the this value.
        // 2. Perform ? RequireInternalSlot(collator, [[InitializedCollator]]).
        let this = this.as_object().ok_or_else(|| {
            JsNativeError::typ()
                .with_message("`resolvedOptions` can only be called on a `Collator` object")
        })?;
        let collator_obj = this.clone();
        let mut collator = this.downcast_mut::<Self>().ok_or_else(|| {
            JsNativeError::typ()
                .with_message("`resolvedOptions` can only be called on a `Collator` object")
        })?;

        // 3. If collator.[[BoundCompare]] is undefined, then
        //     a. Let F be a new built-in function object as defined in 10.3.3.1.
        //     b. Set F.[[Collator]] to collator.
        //     c. Set collator.[[BoundCompare]] to F.
        let bound_compare = if let Some(f) = collator.bound_compare.clone() {
            f
        } else {
            let bound_compare = FunctionObjectBuilder::new(
                context.realm(),
                // 10.3.3.1. Collator Compare Functions
                // https://tc39.es/ecma402/#sec-collator-compare-functions
                NativeFunction::from_copy_closure_with_captures(
                    |_, args, collator, context| {
                        // 1. Let collator be F.[[Collator]].
                        // 2. Assert: Type(collator) is Object and collator has an [[InitializedCollator]] internal slot.
                        let collator = collator
                            .downcast_ref::<Self>()
                            .expect("checked above that the object was a collator object");

                        // 3. If x is not provided, let x be undefined.
                        // 5. Let X be ? ToString(x).
                        let x = args
                            .get_or_undefined(0)
                            .to_string(context)?
                            .iter()
                            .collect::<Vec<_>>();

                        // 4. If y is not provided, let y be undefined.
                        // 6. Let Y be ? ToString(y).
                        let y = args
                            .get_or_undefined(1)
                            .to_string(context)?
                            .iter()
                            .collect::<Vec<_>>();

                        // 7. Return CompareStrings(collator, X, Y).

                        let result = collator.collator.as_borrowed().compare_utf16(&x, &y) as i32;

                        Ok(result.into())
                    },
                    collator_obj,
                ),
            )
            .length(2)
            .build();

            collator.bound_compare = Some(bound_compare.clone());
            bound_compare
        };

        // 4. Return collator.[[BoundCompare]].
        Ok(bound_compare.into())
    }

    /// [`Intl.Collator.prototype.resolvedOptions ( )`][spec].
    ///
    /// Returns a new object with properties reflecting the locale and collation options computed
    /// during initialization of this `Intl.Collator` object.
    ///
    /// More information:
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma402/#sec-intl.collator.prototype.resolvedoptions
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Intl/Collator/resolvedOptions
    fn resolved_options(this: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
        // 1. Let collator be the this value.
        // 2. Perform ? RequireInternalSlot(collator, [[InitializedCollator]]).
        let collator = this
            .as_object()
            .and_then(JsObject::downcast_ref::<Self>)
            .ok_or_else(|| {
                JsNativeError::typ()
                    .with_message("`resolvedOptions` can only be called on a `Collator` object")
            })?;

        // 3. Let options be OrdinaryObjectCreate(%Object.prototype%).
        let options = context
            .intrinsics()
            .templates()
            .ordinary_object()
            .create(OrdinaryObject, vec![]);

        // 4. For each row of Table 4, except the header row, in table order, do
        //     a. Let p be the Property value of the current row.
        //     b. Let v be the value of collator's internal slot whose name is the Internal Slot value of the current row.
        //     c. If the current row has an Extension Key value, then
        //         i. Let extensionKey be the Extension Key value of the current row.
        //         ii. If %Collator%.[[RelevantExtensionKeys]] does not contain extensionKey, then
        //             1. Let v be undefined.
        //     d. If v is not undefined, then
        //         i. Perform ! CreateDataPropertyOrThrow(options, p, v).
        // 5. Return options.
        options
            .create_data_property_or_throw(
                js_string!("locale"),
                js_string!(collator.locale.to_string()),
                context,
            )
            .expect("operation must not fail per the spec");
        options
            .create_data_property_or_throw(
                js_string!("usage"),
                match collator.usage {
                    Usage::Search => js_string!("search"),
                    Usage::Sort => js_string!("sort"),
                },
                context,
            )
            .expect("operation must not fail per the spec");
        options
            .create_data_property_or_throw(
                js_string!("sensitivity"),
                match collator.sensitivity {
                    Sensitivity::Base => js_string!("base"),
                    Sensitivity::Accent => js_string!("accent"),
                    Sensitivity::Case => js_string!("case"),
                    Sensitivity::Variant => js_string!("variant"),
                },
                context,
            )
            .expect("operation must not fail per the spec");
        options
            .create_data_property_or_throw(
                js_string!("ignorePunctuation"),
                collator.ignore_punctuation,
                context,
            )
            .expect("operation must not fail per the spec");
        options
            .create_data_property_or_throw(
                js_string!("collation"),
                js_string!(collator.collation.to_string()),
                context,
            )
            .expect("operation must not fail per the spec");
        options
            .create_data_property_or_throw(js_string!("numeric"), collator.numeric, context)
            .expect("operation must not fail per the spec");
        if let Some(kf) = collator.case_first {
            options
                .create_data_property_or_throw(
                    js_string!("caseFirst"),
                    match kf {
                        CollationCaseFirst::False => js_string!("false"),
                        CollationCaseFirst::Lower => js_string!("lower"),
                        CollationCaseFirst::Upper => js_string!("upper"),
                        _ => unreachable!(),
                    },
                    context,
                )
                .expect("operation must not fail per the spec");
        }

        // 5. Return options.
        Ok(options.into())
    }
}
