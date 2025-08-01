//! Standard library for a JSON query language.
//!
//! The standard library provides a set of filters.
//! These filters are either implemented as definitions or as functions.
//! For example, the standard library provides the `map(f)` filter,
//! which is defined using the more elementary filter `[.[] | f]`.
//!
//! If you want to use the standard library in jaq, then
//! you'll likely only need [`funs`] and [`defs`].
//! Most other functions are relevant if you
//! want to implement your own native filters.
#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

pub mod input;
#[cfg(feature = "math")]
mod math;
#[cfg(feature = "regex")]
mod regex;
#[cfg(feature = "time")]
mod time;

use alloc::string::{String, ToString};
use alloc::{borrow::ToOwned, boxed::Box, vec::Vec};
use jaq_core::box_iter::{box_once, then, BoxIter};
use jaq_core::{load, Bind, Cv, DataT, Error, Exn, Native, RunPtr, ValR, ValT as _, ValX, ValXs};

/// Definitions of the standard library.
pub fn defs() -> impl Iterator<Item = load::parse::Def<&'static str>> {
    load::parse(include_str!("defs.jq"), |p| p.defs())
        .unwrap()
        .into_iter()
}

/// Name, arguments, and implementation of a filter.
pub type Filter<F> = (&'static str, Box<[Bind]>, F);

/// Named filters available by default in jaq
/// which are implemented as native filters, such as `length`, `keys`, ...,
/// but also `now`, `debug`, `fromdateiso8601`, ...
///
/// This is the combination of [`base_funs`] and [`extra_funs`].
/// It does not include filters implemented by definition, such as `map`.
#[cfg(all(
    feature = "std",
    feature = "format",
    feature = "log",
    feature = "math",
    feature = "regex",
    feature = "time",
))]
pub fn funs<D: DataT>() -> impl Iterator<Item = Filter<Native<D>>>
where
    for<'a> D::V<'a>: ValT,
{
    base_funs().chain(extra_funs())
}

/// Minimal set of filters that are generic over the value type.
/// Return the minimal set of named filters available in jaq
/// which are implemented as native filters, such as `length`, `keys`, ...,
/// but not `now`, `debug`, `fromdateiso8601`, ...
///
/// Does not return filters from the standard library, such as `map`.
pub fn base_funs<D: DataT>() -> impl Iterator<Item = Filter<Native<D>>>
where
    for<'a> D::V<'a>: ValT,
{
    let base_run = base_run().into_vec().into_iter().map(run);
    let base_paths = base_paths().into_vec().into_iter().map(paths);
    base_run.chain(base_paths).chain([upd(error())])
}

/// Supplementary set of filters that are generic over the value type.
#[cfg(all(
    feature = "std",
    feature = "format",
    feature = "log",
    feature = "math",
    feature = "regex",
    feature = "time",
))]
pub fn extra_funs<D: DataT>() -> impl Iterator<Item = Filter<Native<D>>>
where
    for<'a> D::V<'a>: ValT,
{
    [std(), format(), math(), regex(), time()]
        .into_iter()
        .flat_map(|fs| fs.into_vec().into_iter().map(run))
        .chain([debug(), stderr()].map(upd))
}

/// Values that the core library can operate on.
pub trait ValT: jaq_core::ValT + Ord + From<f64> {
    /// Convert an array into a sequence.
    ///
    /// This returns the original value as `Err` if it is not an array.
    fn into_seq<S: FromIterator<Self>>(self) -> Result<S, Self>;

    /// Use the value as integer.
    fn as_isize(&self) -> Option<isize>;

    /// Use the value as floating-point number.
    ///
    /// This may fail in more complex ways than [`Self::as_isize`],
    /// because the value may either be
    /// not a number or a number that does not fit into [`f64`].
    fn as_f64(&self) -> Result<f64, Error<Self>>;
}

trait ValTS: jaq_core::ValT {
    fn try_as_str(&self) -> Result<&str, Error<Self>> {
        self.as_str()
            .ok_or_else(|| Error::typ(self.clone(), "string"))
    }

    fn mutate_str(self, f: impl FnOnce(&mut str)) -> ValR<Self> {
        let mut s = self.try_as_str()?.to_owned();
        f(&mut s);
        Ok(Self::from(s))
    }

    fn trim_with(self, f: impl FnOnce(&str) -> &str) -> ValR<Self> {
        let s = self.try_as_str()?;
        let t = f(s);
        Ok(if core::ptr::eq(s, t) {
            // the input was already trimmed, so do not allocate new memory
            self
        } else {
            t.to_string().into()
        })
    }
}
impl<T: jaq_core::ValT> ValTS for T {}

/// Convenience trait for implementing the core functions.
trait ValTx: ValT + Sized {
    fn into_vec(self) -> Result<Vec<Self>, Error<Self>> {
        self.into_seq().map_err(|v| Error::typ(v, "array"))
    }

    fn try_as_isize(&self) -> Result<isize, Error<Self>> {
        self.as_isize()
            .ok_or_else(|| Error::typ(self.clone(), "integer"))
    }

    #[cfg(feature = "math")]
    /// Use as an i32 to be given as an argument to a libm function.
    fn try_as_i32(&self) -> Result<i32, Error<Self>> {
        self.try_as_isize()?.try_into().map_err(Error::str)
    }

    /// Apply a function to an array.
    fn mutate_arr(self, f: impl FnOnce(&mut Vec<Self>)) -> ValR<Self> {
        let mut a = self.into_vec()?;
        f(&mut a);
        Ok(Self::from_iter(a))
    }

    /// Apply a function to an array.
    fn try_mutate_arr<F>(self, f: F) -> ValX<Self>
    where
        F: FnOnce(&mut Vec<Self>) -> Result<(), Exn<Self>>,
    {
        let mut a = self.into_vec()?;
        f(&mut a)?;
        Ok(Self::from_iter(a))
    }

    fn round(self, f: impl FnOnce(f64) -> f64) -> ValR<Self> {
        if self.as_isize().is_some() {
            Ok(self)
        } else {
            Ok(Self::from(f(self.as_f64()?) as isize))
        }
    }
}
impl<T: ValT> ValTx for T {}

/// Convert a filter with a run pointer to a native filter.
pub fn run<D: DataT>((name, arity, run): Filter<RunPtr<D>>) -> Filter<Native<D>> {
    (name, arity, Native::new(run))
}

type RunPathsPtr<D> = (RunPtr<D>, jaq_core::PathsPtr<D>);
type RunPathsUpdatePtr<D> = (RunPtr<D>, jaq_core::PathsPtr<D>, jaq_core::UpdatePtr<D>);

/// Convert a filter with a run and an update pointer to a native filter.
fn paths<D: DataT>((name, arity, (run, paths)): Filter<RunPathsPtr<D>>) -> Filter<Native<D>> {
    (name, arity, Native::new(run).with_paths(paths))
}

/// Convert a filter with a run, a paths, and an update pointer to a native filter.
fn upd<D: DataT>((name, arity, (r, p, u)): Filter<RunPathsUpdatePtr<D>>) -> Filter<Native<D>> {
    (name, arity, Native::new(r).with_paths(p).with_update(u))
}

/// Return all path-value pairs `($p, $v)`, such that `getpath($p) = $v`.
fn path_values<'a, V: ValT + 'a>(v: V, path: Vec<V>) -> BoxIter<'a, (V, V)> {
    let head = (path.iter().cloned().collect(), v.clone());
    let f = move |k| path.iter().cloned().chain([k]).collect();
    let kvs = v.key_values().flatten();
    let kvs: Vec<_> = kvs.map(|(k, v)| (k, v.clone())).collect();
    let tail = kvs.into_iter().flat_map(move |(k, v)| path_values(v, f(k)));
    Box::new(core::iter::once(head).chain(tail))
}

/// Sort array by the given function.
fn sort_by<'a, V: ValT>(xs: &mut [V], f: impl Fn(V) -> ValXs<'a, V>) -> Result<(), Exn<V>> {
    // Some(e) iff an error has previously occurred
    let mut err = None;
    xs.sort_by_cached_key(|x| {
        if err.is_some() {
            return Vec::new();
        };
        match f(x.clone()).collect() {
            Ok(y) => y,
            Err(e) => {
                err = Some(e);
                Vec::new()
            }
        }
    });
    err.map_or(Ok(()), Err)
}

/// Group an array by the given function.
fn group_by<'a, V: ValT>(xs: Vec<V>, f: impl Fn(V) -> ValXs<'a, V>) -> ValX<V> {
    let mut yx: Vec<(Vec<V>, V)> = xs
        .into_iter()
        .map(|x| Ok((f(x.clone()).collect::<Result<_, _>>()?, x)))
        .collect::<Result<_, Exn<_>>>()?;

    yx.sort_by(|(y1, _), (y2, _)| y1.cmp(y2));

    let mut grouped = Vec::new();
    let mut yx = yx.into_iter();
    if let Some((mut group_y, first_x)) = yx.next() {
        let mut group = Vec::from([first_x]);
        for (y, x) in yx {
            if group_y != y {
                grouped.push(V::from_iter(core::mem::take(&mut group)));
                group_y = y;
            }
            group.push(x);
        }
        if !group.is_empty() {
            grouped.push(V::from_iter(group));
        }
    }

    Ok(V::from_iter(grouped))
}

/// Get the minimum or maximum element from an array according to the given function.
fn cmp_by<'a, V: Clone, F, R>(xs: Vec<V>, f: F, replace: R) -> Result<Option<V>, Exn<V>>
where
    F: Fn(V) -> ValXs<'a, V>,
    R: Fn(&[V], &[V]) -> bool,
{
    let iter = xs.into_iter();
    let mut iter = iter.map(|x| (x.clone(), f(x).collect::<Result<Vec<_>, _>>()));
    let (mut mx, mut my) = if let Some((x, y)) = iter.next() {
        (x, y?)
    } else {
        return Ok(None);
    };
    for (x, y) in iter {
        let y = y?;
        if replace(&my, &y) {
            (mx, my) = (x, y);
        }
    }
    Ok(Some(mx))
}

/// Convert a string into an array of its Unicode codepoints.
fn explode<V: ValT>(s: &str) -> impl Iterator<Item = ValR<V>> + '_ {
    // conversion from u32 to isize may fail on 32-bit systems for high values of c
    let conv = |c: char| Ok(isize::try_from(c as u32).map_err(Error::str)?.into());
    s.chars().map(conv)
}

/// Convert an array of Unicode codepoints into a string.
fn implode<V: ValT>(xs: &[V]) -> Result<String, Error<V>> {
    xs.iter().map(as_codepoint).collect()
}

/// If the value is an integer representing a valid Unicode codepoint, return it, else fail.
fn as_codepoint<V: ValT>(v: &V) -> Result<char, Error<V>> {
    let i = v.try_as_isize()?;
    // conversion from isize to u32 may fail on 64-bit systems for high values of c
    let u = u32::try_from(i).map_err(Error::str)?;
    // may fail e.g. on `[1114112] | implode`
    char::from_u32(u).ok_or_else(|| Error::str(format_args!("cannot use {u} as character")))
}

/// This implements a ~10x faster version of:
/// ~~~ text
/// def range($from; $to; $by): $from |
///    if $by > 0 then while(.  < $to; . + $by)
///  elif $by < 0 then while(.  > $to; . + $by)
///    else            while(. != $to; . + $by)
///    end;
/// ~~~
fn range<V: ValT>(mut from: ValX<V>, to: V, by: V) -> impl Iterator<Item = ValX<V>> {
    use core::cmp::Ordering::{Equal, Greater, Less};
    let cmp = by.partial_cmp(&V::from(0)).unwrap_or(Equal);
    core::iter::from_fn(move || match from.clone() {
        Ok(x) => match cmp {
            Greater => x < to,
            Less => x > to,
            Equal => x != to,
        }
        .then(|| core::mem::replace(&mut from, (x + by.clone()).map_err(Exn::from))),
        e @ Err(_) => {
            // return None after the error
            from = Ok(to.clone());
            Some(e)
        }
    })
}

fn once_or_empty<'a, T: 'a, E: 'a>(r: Result<Option<T>, E>) -> BoxIter<'a, Result<T, E>> {
    Box::new(r.transpose().into_iter())
}

/// Box Once and Map Errors to exceptions.
fn bome<'a, V: 'a>(r: ValR<V>) -> ValXs<'a, V> {
    box_once(r.map_err(Exn::from))
}

/// Create a filter that takes a single variable argument and whose output is given by
/// the function `f` that takes the input value and the value of the variable.
pub fn unary<'a, D: DataT>(
    mut cv: Cv<'a, D>,
    f: impl Fn(D::V<'a>, D::V<'a>) -> ValR<D::V<'a>> + 'a,
) -> ValXs<'a, D::V<'a>> {
    bome(f(cv.1, cv.0.pop_var()))
}

/// Creates `n` variable arguments.
pub fn v(n: usize) -> Box<[Bind]> {
    core::iter::repeat(Bind::Var(())).take(n).collect()
}

#[allow(clippy::unit_arg)]
fn base_run<D: DataT>() -> Box<[Filter<RunPtr<D>>]>
where
    for<'a> D::V<'a>: ValT,
{
    let f = || [Bind::Fun(())].into();
    Box::new([
        ("path", f(), |mut cv| {
            let (f, fc) = cv.0.pop_fun();
            let cvp = (fc, (cv.1, Default::default()));
            Box::new(f.paths(cvp).map(|vp| {
                vp.map(|(_v, path)| {
                    let mut path: Vec<_> = path.iter().cloned().collect();
                    path.reverse();
                    path.into_iter().collect()
                })
            }))
        }),
        ("floor", v(0), |cv| bome(cv.1.round(f64::floor))),
        ("round", v(0), |cv| bome(cv.1.round(f64::round))),
        ("ceil", v(0), |cv| bome(cv.1.round(f64::ceil))),
        ("utf8bytelength", v(0), |cv| {
            bome(cv.1.try_as_str().map(|s| (s.len() as isize).into()))
        }),
        ("explode", v(0), |cv| {
            bome(cv.1.try_as_str().and_then(|s| explode(s).collect()))
        }),
        ("implode", v(0), |cv| {
            bome(cv.1.into_vec().and_then(|s| implode(&s)).map(D::V::from))
        }),
        ("ascii_downcase", v(0), |cv| {
            bome(cv.1.mutate_str(str::make_ascii_lowercase))
        }),
        ("ascii_upcase", v(0), |cv| {
            bome(cv.1.mutate_str(str::make_ascii_uppercase))
        }),
        ("reverse", v(0), |cv| bome(cv.1.mutate_arr(|a| a.reverse()))),
        ("keys_unsorted", v(0), |cv| {
            bome(cv.1.key_values().map(|kv| kv.map(|(k, _v)| k)).collect())
        }),
        ("path_values", v(0), |cv| {
            let pair = |(p, v)| Ok([p, v].into_iter().collect());
            Box::new(path_values(cv.1, Vec::new()).skip(1).map(pair))
        }),
        ("paths", v(0), |cv| {
            Box::new(path_values(cv.1, Vec::new()).skip(1).map(|(p, _v)| Ok(p)))
        }),
        ("sort", v(0), |cv| bome(cv.1.mutate_arr(|a| a.sort()))),
        ("sort_by", f(), |mut cv| {
            let (f, fc) = cv.0.pop_fun();
            let f = move |v| f.run((fc.clone(), v));
            box_once(cv.1.try_mutate_arr(|a| sort_by(a, f)))
        }),
        ("group_by", f(), |mut cv| {
            let (f, fc) = cv.0.pop_fun();
            let f = move |v| f.run((fc.clone(), v));
            box_once((|| group_by(cv.1.into_vec()?, f))())
        }),
        ("min_by_or_empty", f(), |mut cv| {
            let (f, fc) = cv.0.pop_fun();
            let f = move |a| cmp_by(a, |v| f.run((fc.clone(), v)), |my, y| y < my);
            once_or_empty(cv.1.into_vec().map_err(Exn::from).and_then(f))
        }),
        ("max_by_or_empty", f(), |mut cv| {
            let (f, fc) = cv.0.pop_fun();
            let f = move |a| cmp_by(a, |v| f.run((fc.clone(), v)), |my, y| y >= my);
            once_or_empty(cv.1.into_vec().map_err(Exn::from).and_then(f))
        }),
        ("range", v(3), |mut cv| {
            let by = cv.0.pop_var();
            let to = cv.0.pop_var();
            let from = cv.0.pop_var();
            Box::new(range(Ok(from), to, by))
        }),
        ("startswith", v(1), |cv| {
            unary(cv, |v, s| {
                Ok(v.try_as_str()?.starts_with(s.try_as_str()?).into())
            })
        }),
        ("endswith", v(1), |cv| {
            unary(cv, |v, s| {
                Ok(v.try_as_str()?.ends_with(s.try_as_str()?).into())
            })
        }),
        ("ltrimstr", v(1), |cv| {
            unary(cv, |v, pre| {
                Ok(v.try_as_str()?
                    .strip_prefix(pre.try_as_str()?)
                    .map_or_else(|| v.clone(), |s| D::V::from(s.to_owned())))
            })
        }),
        ("rtrimstr", v(1), |cv| {
            unary(cv, |v, suf| {
                Ok(v.try_as_str()?
                    .strip_suffix(suf.try_as_str()?)
                    .map_or_else(|| v.clone(), |s| D::V::from(s.to_owned())))
            })
        }),
        ("trim", v(0), |cv| bome(cv.1.trim_with(str::trim))),
        ("ltrim", v(0), |cv| bome(cv.1.trim_with(str::trim_start))),
        ("rtrim", v(0), |cv| bome(cv.1.trim_with(str::trim_end))),
        ("escape_csv", v(0), |cv| {
            bome(cv.1.try_as_str().map(|s| s.replace('"', "\"\"").into()))
        }),
        ("escape_sh", v(0), |cv| {
            bome(cv.1.try_as_str().map(|s| s.replace('\'', r"'\''").into()))
        }),
    ])
}

macro_rules! first {
    ( $run:ident ) => {
        |mut cv| {
            let (f, fc) = cv.0.pop_fun();
            Box::new(f.$run((fc, cv.1)).next().into_iter())
        }
    };
}
macro_rules! last {
    ( $run:ident ) => {
        |mut cv| {
            let (f, fc) = cv.0.pop_fun();
            once_or_empty(f.$run((fc, cv.1)).try_fold(None, |_, x| x.map(Some)))
        }
    };
}
macro_rules! limit {
    ( $run:ident ) => {
        |mut cv| {
            let (f, fc) = cv.0.pop_fun();
            let n = cv.0.pop_var();
            let pos = |n: isize| n.try_into().unwrap_or(0usize);
            then(n.try_as_isize().map_err(Exn::from), |n| match pos(n) {
                0 => Box::new(core::iter::empty()),
                n => Box::new(f.$run((fc, cv.1)).take(n)),
            })
        }
    };
}

fn base_paths<D: DataT>() -> Box<[Filter<RunPathsPtr<D>>]>
where
    for<'a> D::V<'a>: ValT,
{
    let f = || [Bind::Fun(())].into();
    let vf = || [Bind::Var(()), Bind::Fun(())].into();
    Box::new([
        ("first", f(), (first!(run), first!(paths))),
        ("last", f(), (last!(run), last!(paths))),
        ("limit", vf(), (limit!(run), limit!(paths))),
    ])
}

#[cfg(feature = "std")]
fn now<V: From<String>>() -> Result<f64, Error<V>> {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|x| x.as_secs_f64())
        .map_err(Error::str)
}

#[cfg(feature = "std")]
fn std<D: DataT>() -> Box<[Filter<RunPtr<D>>]>
where
    for<'a> D::V<'a>: ValT,
{
    use std::env::vars;
    Box::new([
        ("env", v(0), |_| {
            bome(D::V::from_map(
                vars().map(|(k, v)| (D::V::from(k), D::V::from(v))),
            ))
        }),
        ("now", v(0), |_| bome(now().map(D::V::from))),
        ("halt", v(0), |_| std::process::exit(0)),
        ("halt_error", v(1), |mut cv| {
            bome(cv.0.pop_var().try_as_isize().map(|exit_code| {
                if let Some(s) = cv.1.as_str() {
                    std::print!("{s}");
                } else {
                    std::println!("{}", cv.1);
                }
                std::process::exit(exit_code as i32)
            }))
        }),
    ])
}

#[cfg(feature = "format")]
fn replace(s: &str, patterns: &[&str], replacements: &[&str]) -> String {
    let ac = aho_corasick::AhoCorasick::new(patterns).unwrap();
    ac.replace_all(s, replacements)
}

#[cfg(feature = "format")]
fn format<D: DataT>() -> Box<[Filter<RunPtr<D>>]>
where
    for<'a> D::V<'a>: ValT,
{
    Box::new([
        ("escape_html", v(0), |cv| {
            let pats = ["<", ">", "&", "\'", "\""];
            let reps = ["&lt;", "&gt;", "&amp;", "&apos;", "&quot;"];
            bome(cv.1.try_as_str().map(|s| replace(s, &pats, &reps).into()))
        }),
        ("escape_tsv", v(0), |cv| {
            let pats = ["\n", "\r", "\t", "\\", "\0"];
            let reps = ["\\n", "\\r", "\\t", "\\\\", "\\0"];
            bome(cv.1.try_as_str().map(|s| replace(s, &pats, &reps).into()))
        }),
        ("encode_uri", v(0), |cv| {
            use urlencoding::encode;
            bome(cv.1.try_as_str().map(|s| encode(s).into_owned().into()))
        }),
        ("decode_uri", v(0), |cv| {
            use urlencoding::decode;
            bome(cv.1.try_as_str().and_then(|s| {
                let d = decode(s).map_err(Error::str)?;
                Ok(d.into_owned().into())
            }))
        }),
        ("encode_base64", v(0), |cv| {
            use base64::{engine::general_purpose::STANDARD, Engine};
            bome(cv.1.try_as_str().map(|s| STANDARD.encode(s).into()))
        }),
        ("decode_base64", v(0), |cv| {
            use base64::{engine::general_purpose::STANDARD, Engine};
            use core::str::from_utf8;
            bome(cv.1.try_as_str().and_then(|s| {
                let d = STANDARD.decode(s).map_err(Error::str)?;
                Ok(from_utf8(&d).map_err(Error::str)?.to_owned().into())
            }))
        }),
    ])
}

#[cfg(feature = "math")]
fn math<D: DataT>() -> Box<[Filter<RunPtr<D>>]>
where
    for<'a> D::V<'a>: ValT,
{
    let rename = |name, (_name, arity, f): Filter<RunPtr<D>>| (name, arity, f);
    Box::new([
        math::f_f!(acos),
        math::f_f!(acosh),
        math::f_f!(asin),
        math::f_f!(asinh),
        math::f_f!(atan),
        math::f_f!(atanh),
        math::f_f!(cbrt),
        math::f_f!(cos),
        math::f_f!(cosh),
        math::f_f!(erf),
        math::f_f!(erfc),
        math::f_f!(exp),
        math::f_f!(exp10),
        math::f_f!(exp2),
        math::f_f!(expm1),
        math::f_f!(fabs),
        math::f_fi!(frexp),
        math::f_i!(ilogb),
        math::f_f!(j0),
        math::f_f!(j1),
        math::f_f!(lgamma),
        math::f_f!(log),
        math::f_f!(log10),
        math::f_f!(log1p),
        math::f_f!(log2),
        // logb is implemented in jaq-std
        math::f_ff!(modf),
        rename("nearbyint", math::f_f!(round)),
        // pow10 is implemented in jaq-std
        math::f_f!(rint),
        // significand is implemented in jaq-std
        math::f_f!(sin),
        math::f_f!(sinh),
        math::f_f!(sqrt),
        math::f_f!(tan),
        math::f_f!(tanh),
        math::f_f!(tgamma),
        math::f_f!(trunc),
        math::f_f!(y0),
        math::f_f!(y1),
        math::ff_f!(atan2),
        math::ff_f!(copysign),
        // drem is implemented in jaq-std
        math::ff_f!(fdim),
        math::ff_f!(fmax),
        math::ff_f!(fmin),
        math::ff_f!(fmod),
        math::ff_f!(hypot),
        math::if_f!(jn),
        math::fi_f!(ldexp),
        math::ff_f!(nextafter),
        // nexttoward is implemented in jaq-std
        math::ff_f!(pow),
        math::ff_f!(remainder),
        // scalb is implemented in jaq-std
        rename("scalbln", math::fi_f!(scalbn)),
        math::if_f!(yn),
        math::fff_f!(fma),
    ])
}

#[cfg(feature = "regex")]
fn re<'a, D: DataT>(s: bool, m: bool, mut cv: Cv<'a, D>) -> ValR<D::V<'a>> {
    let flags = cv.0.pop_var();
    let re = cv.0.pop_var();

    use crate::regex::Part::{Matches, Mismatch};
    let fail_flag = |e| Error::str(format_args!("invalid regex flag: {e}"));
    let fail_re = |e| Error::str(format_args!("invalid regex: {e}"));

    let flags = regex::Flags::new(flags.try_as_str()?).map_err(fail_flag)?;
    let re = flags.regex(re.try_as_str()?).map_err(fail_re)?;
    let out = regex::regex(cv.1.try_as_str()?, &re, flags, (s, m));
    let out = out.into_iter().map(|out| match out {
        Matches(ms) => ms.into_iter().map(|m| D::V::from_map(m.fields())).collect(),
        Mismatch(s) => Ok(D::V::from(s.to_string())),
    });
    out.collect()
}

#[cfg(feature = "regex")]
fn regex<D: DataT>() -> Box<[Filter<RunPtr<D>>]> {
    let vv = || [Bind::Var(()), Bind::Var(())].into();
    Box::new([
        ("matches", vv(), |cv| bome(re(false, true, cv))),
        ("split_matches", vv(), |cv| bome(re(true, true, cv))),
        ("split_", vv(), |cv| bome(re(true, false, cv))),
    ])
}

#[cfg(feature = "time")]
fn time<D: DataT>() -> Box<[Filter<RunPtr<D>>]>
where
    for<'a> D::V<'a>: ValT,
{
    use chrono::{Local, Utc};
    Box::new([
        ("fromdateiso8601", v(0), |cv| {
            bome(cv.1.try_as_str().and_then(time::from_iso8601))
        }),
        ("todateiso8601", v(0), |cv| {
            bome(time::to_iso8601(&cv.1).map(D::V::from))
        }),
        ("strftime", v(1), |cv| {
            unary(cv, |v, fmt| time::strftime(&v, fmt.try_as_str()?, Utc))
        }),
        ("strflocaltime", v(1), |cv| {
            unary(cv, |v, fmt| time::strftime(&v, fmt.try_as_str()?, Local))
        }),
        ("gmtime", v(0), |cv| bome(time::gmtime(&cv.1, Utc))),
        ("localtime", v(0), |cv| bome(time::gmtime(&cv.1, Local))),
        ("strptime", v(1), |cv| {
            unary(cv, |v, fmt| {
                time::strptime(v.try_as_str()?, fmt.try_as_str()?)
            })
        }),
        ("mktime", v(0), |cv| bome(time::mktime(&cv.1))),
    ])
}

fn error<D: DataT>() -> Filter<RunPathsUpdatePtr<D>> {
    (
        "error",
        v(0),
        (
            |cv| bome(Err(Error::new(cv.1))),
            |cv| box_once(Err(Exn::from(Error::new(cv.1 .0)))),
            |cv, _| bome(Err(Error::new(cv.1))),
        ),
    )
}

#[cfg(feature = "log")]
/// Construct a filter that applies an effect function before returning its input.
macro_rules! id_with {
    ( $eff:expr ) => {
        (
            |cv| {
                $eff(&cv.1);
                box_once(Ok(cv.1))
            },
            |cv| {
                $eff(&cv.1 .0);
                box_once(Ok(cv.1))
            },
            |cv, f| {
                $eff(&cv.1);
                f(cv.1)
            },
        )
    };
}

#[cfg(feature = "log")]
fn debug<D: DataT>() -> Filter<RunPathsUpdatePtr<D>> {
    ("debug", v(0), id_with!(|x| log::debug!("{x}")))
}

#[cfg(feature = "log")]
fn stderr<D: DataT>() -> Filter<RunPathsUpdatePtr<D>> {
    fn eprint_raw<V: jaq_core::ValT>(v: &V) {
        if let Some(s) = v.as_str() {
            log::error!("{s}")
        } else {
            log::error!("{v}")
        }
    }
    ("stderr", v(0), id_with!(eprint_raw))
}
