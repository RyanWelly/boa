//! This module implements the global `Intl.DateTimeFormat` object.
//!
//! `Intl.DateTimeFormat` is a built-in object that has properties and methods for date and time i18n.
//!
//! More information:
//!  - [ECMAScript reference][spec]
//!
//! [spec]: https://tc39.es/ecma402/#datetimeformat-objects

use crate::{
    builtins::{
        options::OptionType, BuiltInBuilder, BuiltInConstructor, BuiltInObject, IntrinsicObject,
        OrdinaryObject,
    },
    context::intrinsics::{Intrinsics, StandardConstructor, StandardConstructors},
    error::JsNativeError,
    js_string,
    object::{internal_methods::get_prototype_from_constructor, JsObject},
    realm::Realm,
    string::StaticJsStrings,
    Context, JsData, JsResult, JsString, JsValue,
};

use boa_gc::{Finalize, Trace};
use boa_profiler::Profiler;
use icu_calendar::preferences::CalendarAlgorithm;
use icu_datetime::preferences::HourCycle;
use icu_locale::extensions::unicode::Value;

/// JavaScript `Intl.DateTimeFormat` object.
#[derive(Debug, Clone, Trace, Finalize, JsData)]
pub(crate) struct DateTimeFormat {
    initialized: bool,
    locale: JsString,
    calendar: JsString,
    numbering_system: JsString,
    time_zone: JsString,
    weekday: JsString,
    era: JsString,
    year: JsString,
    month: JsString,
    day: JsString,
    day_period: JsString,
    hour: JsString,
    minute: JsString,
    second: JsString,
    fractional_second_digits: JsString,
    time_zone_name: JsString,
    hour_cycle: JsString,
    pattern: JsString,
    bound_format: JsString,
}

impl IntrinsicObject for DateTimeFormat {
    fn init(realm: &Realm) {
        let _timer = Profiler::global().start_event(std::any::type_name::<Self>(), "init");

        BuiltInBuilder::from_standard_constructor::<Self>(realm).build();
    }

    fn get(intrinsics: &Intrinsics) -> JsObject {
        Self::STANDARD_CONSTRUCTOR(intrinsics.constructors()).constructor()
    }
}

impl BuiltInObject for DateTimeFormat {
    const NAME: JsString = StaticJsStrings::DATE_TIME_FORMAT;
}

impl BuiltInConstructor for DateTimeFormat {
    const LENGTH: usize = 0;
    const P: usize = 0;
    const SP: usize = 0;

    const STANDARD_CONSTRUCTOR: fn(&StandardConstructors) -> &StandardConstructor =
        StandardConstructors::date_time_format;
    /// The `Intl.DateTimeFormat` constructor is the `%DateTimeFormat%` intrinsic object and a standard built-in property of the `Intl` object.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma402/#datetimeformat-objects
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Intl/DateTimeFormat
    fn constructor(
        new_target: &JsValue,
        _args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        // 1. If NewTarget is undefined, let newTarget be the active function object, else let newTarget be NewTarget.
        let new_target = &if new_target.is_undefined() {
            context
                .active_function_object()
                .unwrap_or_else(|| {
                    context
                        .intrinsics()
                        .constructors()
                        .date_time_format()
                        .constructor()
                })
                .into()
        } else {
            new_target.clone()
        };
        let prototype = get_prototype_from_constructor(
            new_target,
            StandardConstructors::date_time_format,
            context,
        )?;
        // 2. Let dateTimeFormat be ? OrdinaryCreateFromConstructor(newTarget, "%DateTimeFormat.prototype%",
        // « [[InitializedDateTimeFormat]], [[Locale]], [[Calendar]], [[NumberingSystem]], [[TimeZone]], [[Weekday]],
        // [[Era]], [[Year]], [[Month]], [[Day]], [[DayPeriod]], [[Hour]], [[Minute]], [[Second]],
        // [[FractionalSecondDigits]], [[TimeZoneName]], [[HourCycle]], [[Pattern]], [[BoundFormat]] »).
        let date_time_format = JsObject::from_proto_and_data_with_shared_shape(
            context.root_shape(),
            prototype,
            Self {
                initialized: true,
                locale: js_string!("en-US"),
                calendar: js_string!("gregory"),
                numbering_system: js_string!("arab"),
                time_zone: js_string!("UTC"),
                weekday: js_string!("narrow"),
                era: js_string!("narrow"),
                year: js_string!("numeric"),
                month: js_string!("narrow"),
                day: js_string!("numeric"),
                day_period: js_string!("narrow"),
                hour: js_string!("numeric"),
                minute: js_string!("numeric"),
                second: js_string!("numeric"),
                fractional_second_digits: js_string!(),
                time_zone_name: js_string!(),
                hour_cycle: js_string!("h24"),
                pattern: js_string!("{hour}:{minute}"),
                bound_format: js_string!("undefined"),
            },
        );

        // TODO 3. Perform ? InitializeDateTimeFormat(dateTimeFormat, locales, options).
        // TODO 4. If the implementation supports the normative optional constructor mode of 4.3 Note 1, then
        // TODO a. Let this be the this value.
        // TODO b. Return ? ChainDateTimeFormat(dateTimeFormat, NewTarget, this).

        // 5. Return dateTimeFormat.
        Ok(date_time_format.into())
    }
}

/// Represents the `required` and `defaults` arguments in the abstract operation
/// `toDateTimeOptions`.
///
/// Since `required` and `defaults` differ only in the `any` and `all` variants,
/// we combine both in a single variant `AnyAll`.
#[allow(unused)]
#[derive(Debug, PartialEq)]
pub(crate) enum DateTimeReqs {
    Date,
    Time,
    AnyAll,
}

/// The abstract operation `toDateTimeOptions` is called with arguments `options`, `required` and
/// `defaults`.
///
/// More information:
///  - [ECMAScript reference][spec]
///
/// [spec]: https://tc39.es/ecma402/#sec-todatetimeoptions
#[allow(unused)]
pub(crate) fn to_date_time_options(
    options: &JsValue,
    required: &DateTimeReqs,
    defaults: &DateTimeReqs,
    context: &mut Context,
) -> JsResult<JsObject> {
    // 1. If options is undefined, let options be null;
    // otherwise let options be ? ToObject(options).
    // 2. Let options be ! OrdinaryObjectCreate(options).
    let options = if options.is_undefined() {
        None
    } else {
        Some(options.to_object(context)?)
    };
    let options = JsObject::from_proto_and_data_with_shared_shape(
        context.root_shape(),
        options,
        OrdinaryObject,
    );

    // 3. Let needDefaults be true.
    let mut need_defaults = true;

    // 4. If required is "date" or "any", then
    if [DateTimeReqs::Date, DateTimeReqs::AnyAll].contains(required) {
        // a. For each property name prop of « "weekday", "year", "month", "day" », do
        for property in [
            js_string!("weekday"),
            js_string!("year"),
            js_string!("month"),
            js_string!("day"),
        ] {
            // i. Let value be ? Get(options, prop).
            let value = options.get(property, context)?;

            // ii. If value is not undefined, let needDefaults be false.
            if !value.is_undefined() {
                need_defaults = false;
            }
        }
    }

    // 5. If required is "time" or "any", then
    if [DateTimeReqs::Time, DateTimeReqs::AnyAll].contains(required) {
        // a. For each property name prop of « "dayPeriod", "hour", "minute", "second",
        // "fractionalSecondDigits" », do
        for property in [
            js_string!("dayPeriod"),
            js_string!("hour"),
            js_string!("minute"),
            js_string!("second"),
            js_string!("fractionalSecondDigits"),
        ] {
            // i. Let value be ? Get(options, prop).
            let value = options.get(property, context)?;

            // ii. If value is not undefined, let needDefaults be false.
            if !value.is_undefined() {
                need_defaults = false;
            }
        }
    }

    // 6. Let dateStyle be ? Get(options, "dateStyle").
    let date_style = options.get(js_string!("dateStyle"), context)?;

    // 7. Let timeStyle be ? Get(options, "timeStyle").
    let time_style = options.get(js_string!("timeStyle"), context)?;

    // 8. If dateStyle is not undefined or timeStyle is not undefined, let needDefaults be false.
    if !date_style.is_undefined() || !time_style.is_undefined() {
        need_defaults = false;
    }

    // 9. If required is "date" and timeStyle is not undefined, then
    if required == &DateTimeReqs::Date && !time_style.is_undefined() {
        // a. Throw a TypeError exception.
        return Err(JsNativeError::typ()
            .with_message("'date' is required, but timeStyle was defined")
            .into());
    }

    // 10. If required is "time" and dateStyle is not undefined, then
    if required == &DateTimeReqs::Time && !date_style.is_undefined() {
        // a. Throw a TypeError exception.
        return Err(JsNativeError::typ()
            .with_message("'time' is required, but dateStyle was defined")
            .into());
    }

    // 11. If needDefaults is true and defaults is either "date" or "all", then
    if need_defaults && [DateTimeReqs::Date, DateTimeReqs::AnyAll].contains(defaults) {
        // a. For each property name prop of « "year", "month", "day" », do
        for property in [js_string!("year"), js_string!("month"), js_string!("day")] {
            // i. Perform ? CreateDataPropertyOrThrow(options, prop, "numeric").
            options.create_data_property_or_throw(property, js_string!("numeric"), context)?;
        }
    }

    // 12. If needDefaults is true and defaults is either "time" or "all", then
    if need_defaults && [DateTimeReqs::Time, DateTimeReqs::AnyAll].contains(defaults) {
        // a. For each property name prop of « "hour", "minute", "second" », do
        for property in [
            js_string!("hour"),
            js_string!("minute"),
            js_string!("second"),
        ] {
            // i. Perform ? CreateDataPropertyOrThrow(options, prop, "numeric").
            options.create_data_property_or_throw(property, js_string!("numeric"), context)?;
        }
    }

    // 13. Return options.
    Ok(options)
}

impl OptionType for CalendarAlgorithm {
    fn from_value(value: JsValue, context: &mut Context) -> JsResult<Self> {
        let s = value.to_string(context)?.to_std_string_escaped();
        Value::try_from_str(&s)
            .ok()
            .and_then(|v| CalendarAlgorithm::try_from(&v).ok())
            .ok_or_else(|| {
                JsNativeError::range()
                    .with_message(format!("provided calendar `{s}` is invalid"))
                    .into()
            })
    }
}

// TODO: track https://github.com/unicode-org/icu4x/issues/6597 and
// https://github.com/tc39/ecma402/issues/1002 for resolution on
// `HourCycle::H24`.
impl OptionType for HourCycle {
    fn from_value(value: JsValue, context: &mut Context) -> JsResult<Self> {
        match value.to_string(context)?.to_std_string_escaped().as_str() {
            "h11" => Ok(HourCycle::H11),
            "h12" => Ok(HourCycle::H12),
            "h23" => Ok(HourCycle::H23),
            _ => Err(JsNativeError::range()
                .with_message("provided hour cycle was not `h11`, `h12` or `h23`")
                .into()),
        }
    }
}
