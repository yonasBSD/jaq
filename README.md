# jaq

![Build status](https://github.com/01mf02/jaq/actions/workflows/check.yml/badge.svg)
[![Crates.io](https://img.shields.io/crates/v/jaq-core.svg)](https://crates.io/crates/jaq-core)
[![Documentation](https://docs.rs/jaq-core/badge.svg)](https://docs.rs/jaq-core)
[![Rust 1.69+](https://img.shields.io/badge/rust-1.69+-orange.svg)](https://www.rust-lang.org)

jaq (pronounced /ʒaːk/, like *Jacques*[^jacques]) is a clone of the JSON data processing tool [jq].
jaq aims to support a large subset of jq's syntax and operations.

You can try jaq online on the [jaq playground](https://gedenkt.at/jaq/).
Instructions for the playground can be found [here](jaq-play/).

jaq focuses on three goals:

* **Correctness**:
  jaq aims to provide a more correct and predictable implementation of jq,
  while preserving compatibility with jq in most cases.
* **Performance**:
  I created jaq originally because I was bothered by
  [the long start-up time of jq 1.6](https://github.com/jqlang/jq/issues/1411),
  which amounts to about 50ms on my machine.
  This can be particularly seen when processing a large number of small files.
  Although the startup time has been vastly improved in jq 1.7,
  jaq is still faster than jq on many other [benchmarks](#performance).
* **Simplicity**:
  jaq aims to have a simple and small implementation, in order to
  reduce the potential for bugs and to
  facilitate contributions.

I drew inspiration from another Rust program, namely [jql].
However, unlike jql, jaq aims to closely imitate jq's syntax and semantics.
This should allow users proficient in jq to easily use jaq.

[jq]: https://jqlang.github.io/jq/
[jql]: https://github.com/yamafaktory/jql

[^jacques]: I wanted to create a tool that should be discreet and obliging, like a good waiter.
  And when I think of a typical name for a (French) waiter, to my mind comes "Jacques".
  Later, I found out about the old French word *jacquet*, meaning "squirrel",
  which makes for a nice *ex post* inspiration for the name.



# Installation


## Binaries

You can download binaries for Linux, Mac, and Windows on the [releases page](https://github.com/01mf02/jaq/releases).
On a Linux system, you can download it using the following commands:

    $ curl -fsSL https://github.com/01mf02/jaq/releases/latest/download/jaq-$(uname -m)-unknown-linux-musl -o jaq && chmod +x jaq

You may also install jaq using [homebrew](https://formulae.brew.sh/formula/jaq) on macOS or Linux:

    $ brew install jaq
    $ brew install --HEAD jaq # latest development version

[![Packaging status](https://repology.org/badge/vertical-allrepos/jaq.svg)](https://repology.org/project/jaq/versions)


## From Source

To compile jaq, you need a Rust toolchain.
See <https://rustup.rs/> for instructions.

Any of the following commands install jaq:

    $ cargo install --locked jaq
    $ cargo install --locked --git https://github.com/01mf02/jaq # latest development version

On my system, both commands place the executable at `~/.cargo/bin/jaq`.

If you have cloned this repository, you can also build jaq by executing one of the commands in the cloned repository:

    $ cargo build --release # places binary into target/release/jaq
    $ cargo install --locked --path jaq # installs binary

jaq should work on any system supported by Rust.
If it does not, please file an issue.



# Examples

The following examples should give an impression of what jaq can currently do.
You should obtain the same outputs by replacing jaq with jq.
If not, your filing an issue would be appreciated. :)
The syntax is documented in the [jq manual].

[jq manual]: https://jqlang.github.io/jq/manual/v1.6/

Access a field:

    $ echo '{"a": 1, "b": 2}' | jaq '.a'
    1

Add values:

    $ echo '{"a": 1, "b": 2}' | jaq 'add'
    3

Construct an array from an object in two ways and show that they are equal:

    $ echo '{"a": 1, "b": 2}' | jaq '[.a, .b] == [.[]]'
    true

Apply a filter to all elements of an array and filter the results:

    $ echo '[0, 1, 2, 3]' | jaq 'map(.*2) | [.[] | select(. < 5)]'
    [0, 2, 4]

Read (slurp) input values into an array and get the average of its elements:

    $ echo '1 2 3 4' | jaq -s 'add / length'
    2.5

Repeatedly apply a filter to itself and output the intermediate results:

    $ echo '0' | jaq '[recurse(.+1; . < 3)]'
    [0, 1, 2]

Lazily fold over inputs and output intermediate results:

    $ seq 1000 | jaq -n 'foreach inputs as $x (0; . + $x)'
    1 3 6 10 15 [...]



# Performance

The following evaluation consists of several benchmarks that
allow comparing the performance of jaq, jq, and [gojq].
The `empty` benchmark runs `n` times the filter `empty` with null input,
serving to measure the startup time.
The `bf-fib` benchmark runs a Brainfuck interpreter written in jq,
interpreting a Brainfuck script that produces `n` Fibonacci numbers.
The other benchmarks evaluate various filters with `n` as input;
see [`bench.sh`](bench.sh) for details.

I generated the benchmark data with
`bench.sh target/release/jaq jq-1.8.1 gojq-0.12.17 | tee bench.json`
on a Linux system with an AMD Ryzen 5 5500U.[^binaries]
I then processed the results with a "one-liner" (stretching the term and the line a bit):

    jq -rs '.[] | "|`\(.name)`|\(.n)|" + ([.time[] | min | (.*1000|round)? // "N/A"] | min as $total_min | map(if . == $total_min then "**\(.)**" else "\(.)" end) | join("|"))' bench.json

(Of course, you can also use jaq here instead of jq.)
Finally, I concatenated the table header with the output and piped it through `pandoc -t gfm`.

[^binaries]: The binaries for jq-1.8.1 and gojq-0.12.17 were retrieved from their GitHub release pages.

Table: Evaluation results in milliseconds ("N/A" if error or more than 10 seconds).

| Benchmark       |       n | jaq-2.3 | jq-1.8.1 | gojq-0.12.17 |
|-----------------|--------:|--------:|---------:|-------------:|
| `empty`         |     512 |     330 |      440 |      **290** |
| `bf-fib`        |      13 | **430** |     1110 |          540 |
| `defs`          |  100000 |  **60** |      N/A |         1000 |
| `upto`          |    8192 |   **0** |      470 |          450 |
| `reduce-update` |   16384 |  **10** |      490 |         1200 |
| `reverse`       | 1048576 |  **30** |      500 |          270 |
| `sort`          | 1048576 | **100** |      450 |          540 |
| `group-by`      | 1048576 | **340** |     1750 |         1540 |
| `min-max`       | 1048576 |     190 |  **170** |          260 |
| `add`           | 1048576 | **440** |      570 |         1150 |
| `kv`            |  131072 | **100** |      140 |          270 |
| `kv-update`     |  131072 | **110** |      480 |          480 |
| `kv-entries`    |  131072 | **520** |     1050 |          800 |
| `ex-implode`    | 1048576 | **470** |     1010 |          590 |
| `reduce`        | 1048576 | **720** |      850 |          N/A |
| `try-catch`     | 1048576 | **170** |      220 |          370 |
| `repeat`        | 1048576 | **140** |      690 |          530 |
| `from`          | 1048576 | **280** |      800 |          550 |
| `last`          | 1048576 |  **40** |      160 |          110 |
| `pyramid`       |  524288 |     310 |  **270** |          480 |
| `tree-contains` |      23 |  **60** |      590 |          220 |
| `tree-flatten`  |      17 |     750 |      340 |        **0** |
| `tree-update`   |      17 | **470** |      970 |         1300 |
| `tree-paths`    |      17 | **130** |      250 |          770 |
| `to-fromjson`   |   65536 |  **40** |      370 |          100 |
| `ack`           |       7 | **510** |      540 |         1090 |
| `range-prop`    |     128 |     350 |      270 |      **230** |
| `cumsum`        | 1048576 | **250** |      260 |          460 |
| `cumsum-xy`     | 1048576 |     400 |  **350** |          680 |

The results show that
jaq-2.3 is fastest on 23 benchmarks, whereas
jq-1.8.1 is fastest on 3 benchmark and
gojq-0.12.17 is fastest on 3 benchmarks.
gojq is much faster on `tree-flatten` because it implements the filter `flatten` natively instead of by definition.

[gojq]: https://github.com/itchyny/gojq


# Security

jaq's core has been audited by
[Radically Open Security](https://www.radicallyopensecurity.com/)
as part of an [NLnet](https://nlnet.nl/) grant ---
thanks to both organisations for their support!
The [security audit](https://github.com/01mf02/jaq/releases/download/v2.2.0/jaq.penetration.test.report.2025.1.0.pdf) found
one low severity issue and three issues that are likely not exploitable at all.
As a result of this security audit, all issues were addressed and
several fuzzing targets for jaq were added at `jaq-core/fuzz`.
Before that, jaq's JSON parser [hifijson](https://github.com/01mf02/hifijson/)
already disposed of a fuzzing target.
Finally, jaq disposes of a carefully crafted test suite of more than 500 tests
that is checked at every commit.


# Features

Here is an overview that summarises:

* [x] features already implemented, and
* [ ] features not yet implemented.

[Contributions to extend jaq are highly welcome.](#contributing)


## Basics

- [x] Identity (`.`)
- [x] Recursion (`..`)
- [x] Basic data types (null, boolean, number, string, array, object)
- [x] if-then-else (`if .a < .b then .a else .b end`)
- [x] Folding (`reduce .[] as $x (0; . + $x)`, `foreach .[] as $x (0; . + $x; . + .)`)
- [x] Error handling (`try ... catch ...`)
- [x] Breaking (`label $x | f | ., break $x`)
- [x] String interpolation (`"The successor of \(.) is \(.+1)."`)
- [x] Format strings (`@json`, `@text`, `@csv`, `@tsv`, `@html`, `@sh`, `@base64`, `@base64d`, `@uri`, `@urid`)


## Paths

- [x] Indexing of arrays/objects (`.[0]`, `.a`, `.["a"]`)
- [x] Iterating over arrays/objects (`.[]`)
- [x] Optional indexing/iteration (`.a?`, `.[]?`)
- [x] Array slices (`.[3:7]`, `.[0:-1]`)
- [x] String slices


## Operators

- [x] Composition (`|`)
- [x] Variable binding (`. as $x | $x`)
- [x] Pattern  binding (`. as {a: [$x, {("b", "c"): $y, $z}]} | $x, $y, $z`)
- [x] Concatenation (`,`)
- [x] Plain assignment (`=`)
- [x] Update assignment (`|=`)
- [x] Arithmetic update assignment (`+=`, `-=`, ...)
- [x] Alternation (`//`)
- [x] Logic (`or`, `and`)
- [x] Equality and comparison (`.a == .b`, `.a < .b`)
- [x] Arithmetic (`+`, `-`, `*`, `/`, `%`)
- [x] Negation (`-`)
- [x] Error suppression (`?`)


## Definitions

- [x] Basic definitions (`def map(f): [.[] | f];`)
- [x] Recursive definitions (`def r: r; r`)


## Core filters

- [x] Empty (`empty`)
- [x] Errors (`error`)
- [x] Input (`inputs`)
- [x] Length (`length`, `utf8bytelength`)
- [x] Rounding (`floor`, `round`, `ceil`)
- [x] String <-> JSON (`fromjson`, `tojson`)
- [x] String <-> integers (`explode`, `implode`)
- [x] String normalisation (`ascii_downcase`, `ascii_upcase`)
- [x] String prefix/postfix (`startswith`, `endswith`, `ltrimstr`, `rtrimstr`)
- [x] String whitespace trimming (`trim`, `ltrim`, `rtrim`)
- [x] String splitting (`split("foo")`)
- [x] Array filters (`reverse`, `sort`, `sort_by(-.)`, `group_by`, `min_by`, `max_by`, `bsearch`)
- [x] Stream consumers (`first`, `last`, `range`, `fold`)
- [x] Stream generators (`range`, `recurse`)
- [x] Time (`now`, `fromdateiso8601`, `todateiso8601`)
- [x] More numeric filters (`sqrt`, `sin`, `log`, `pow`, ...) ([list of numeric filters](#numeric-filters))
- [x] More time filters (`strptime`, `strftime`, `strflocaltime`, `mktime`, `gmtime`, and `localtime`)

## Standard filters

These filters are defined via more basic filters.
Their definitions are at [`std.jq`](jaq-std/src/std.jq).

- [x] Undefined (`null`)
- [x] Booleans (`true`, `false`, `not`)
- [x] Special numbers (`nan`, `infinite`, `isnan`, `isinfinite`, `isfinite`, `isnormal`)
- [x] Type (`type`)
- [x] Filtering (`select(. >= 0)`)
- [x] Selection (`values`, `nulls`, `booleans`, `numbers`, `strings`, `arrays`, `objects`, `iterables`, `scalars`)
- [x] Conversion (`tostring`, `tonumber`, `toboolean`)
- [x] Iterable filters (`map(.+1)`, `map_values(.+1)`, `add`, `join("a")`)
- [x] Array filters (`transpose`, `first`, `last`, `nth(10)`, `flatten`, `min`, `max`)
- [x] Object-array conversion (`to_entries`, `from_entries`, `with_entries`)
- [x] Universal/existential (`all`, `any`)
- [x] Recursion (`walk`)
- [x] I/O (`input`)
- [x] Regular expressions (`test`, `scan`, `match`, `capture`, `splits`, `sub`, `gsub`)
- [x] Time (`fromdate`, `todate`)

## Numeric filters

jaq imports many filters from [libm](https://crates.io/crates/libm)
and follows their type signature.

<details><summary>Full list of numeric filters defined in jaq</summary>

Zero-argument filters:

- [x] `acos`
- [x] `acosh`
- [x] `asin`
- [x] `asinh`
- [x] `atan`
- [x] `atanh`
- [x] `cbrt`
- [x] `cos`
- [x] `cosh`
- [x] `erf`
- [x] `erfc`
- [x] `exp`
- [x] `exp10`
- [x] `exp2`
- [x] `expm1`
- [x] `fabs`
- [x] `frexp`, which returns pairs of (float, integer).
- [x] `gamma`
- [x] `ilogb`, which returns integers.
- [x] `j0`
- [x] `j1`
- [x] `lgamma`
- [x] `log`
- [x] `log10`
- [x] `log1p`
- [x] `log2`
- [x] `logb`
- [x] `modf`, which returns pairs of (float, float).
- [x] `nearbyint`
- [x] `pow10`
- [x] `rint`
- [x] `significand`
- [x] `sin`
- [x] `sinh`
- [x] `sqrt`
- [x] `tan`
- [x] `tanh`
- [x] `tgamma`
- [x] `trunc`
- [x] `y0`
- [x] `y1`

Two-argument filters that ignore `.`:

- [x] `atan2`
- [x] `copysign`
- [x] `drem`
- [x] `fdim`
- [x] `fmax`
- [x] `fmin`
- [x] `fmod`
- [x] `hypot`
- [x] `jn`, which takes an integer as first argument.
- [x] `ldexp`, which takes an integer as second argument.
- [x] `nextafter`
- [x] `nexttoward`
- [x] `pow`
- [x] `remainder`
- [x] `scalb`
- [x] `scalbln`, which takes as integer as second argument.
- [x] `yn`, which takes an integer as first argument.

Three-argument filters that ignore `.`:

- [x] `fma`

</details>

## Modules

- [x] `include "path";`
- [x] `import "path" as mod;`
- [x] `import "path" as $data;`

## Advanced features

jaq currently does *not* aim to support several features of jq, such as:

- SQL-style operators
- Streaming



# Differences between jq and jaq


## Numbers

jq uses 64-bit floating-point numbers (floats) for any number.
By contrast, jaq interprets
numbers such as 0   or -42 as machine-sized integers and
numbers such as 0.0 or 3e8 as 64-bit floats.
Many operations in jaq, such as array indexing,
check whether the passed numbers are indeed integer.
The motivation behind this is to avoid
rounding errors that may silently lead to wrong results.
For example:

    $ jq  -n '[0, 1, 2] | .[1.0000000000000001]'
    1
    $ jaq -n '[0, 1, 2] | .[1.0000000000000001]'
    Error: cannot use 1.0 as integer
    $ jaq -n '[0, 1, 2] | .[1]'
    1

The rules of jaq are:

* The sum, difference, product, and remainder of two integers is integer.
* Any other operation between two numbers yields a float.

Examples:

    $ jaq -n '1 + 2'
    3
    $ jaq -n '10 / 2'
    5.0
    $ jaq -n '1.0 + 2'
    3.0

You can convert an integer to a floating-point number e.g.
by adding 0.0, by multiplying with 1.0, or by dividing with 1.
You can convert a floating-point number to an integer by
`round`, `floor`, or `ceil`:

    $ jaq -n '1.2 | [floor, round, ceil]'
    [1, 1, 2]

### NaN and infinity

In jq, division by 0 yields an error, whereas
in jaq, `n / 0` yields `nan` if `n == 0`, `infinite` if `n > 0`, and `-infinite` if `n < 0`.
jaq's behaviour is closer to the IEEE standard for floating-point arithmetic (IEEE 754).


## Assignments

Like jq, jaq allows for assignments of the form `p |= f`.
However, jaq interprets these assignments differently.
Fortunately, in most cases, the result is the same.

In jq, an assignment `p |= f` first constructs paths to all values that match `p`.
*Only then*, it applies the filter `f` to these values.

In jaq, an assignment `p |= f` applies `f` *immediately* to any value matching `p`.
Unlike in jq, assignment does not explicitly construct paths.

jaq's implementation of assignment likely yields higher performance,
because it does not construct paths.
Furthermore, this allows jaq to use multiple outputs of the right-hand side, whereas
jq uses only the first.
For example, `0 | (., .) |= (., .+1)` yields `0 1 1 2` in jaq,
whereas it yields only `0` in jq.
However, `{a: 1} | .a |= (2, 3)` yields `{"a": 2}` in both jaq and jq,
because an object can only associate a single value with any given key,
so we cannot use multiple outputs in a meaningful way here.

Because jaq does not construct paths,
it does not allow some filters on the left-hand side of assignments,
for example `first`, `last`, `limit`:
For example, `[1, 2, 3] | first(.[]) |= .-1`
yields `[0, 2, 3]` in jq, but is invalid in jaq.
Similarly, `[1, 2, 3] | limit(2; .[]) |= .-1`
yields `[0, 1, 3]` in jq, but is invalid in jaq.
(Inconsequentially, jq also does not allow for `last`.)


## Folding

jq and jaq provide filters
`reduce xs as $x (init; update)`,
`foreach xs as $x (init; update)`, and
`foreach xs as $x (init; update; project)`, where
`foreach xs as $x (init; update)` is equivalent to
`foreach xs as $x (init; update; .)`.

In jaq, the output of these filters is defined very simply:
Assuming that `xs` evaluates to `x0`, `x1`, ..., `xn`,
`reduce xs as $x (init; update)` evaluates to

~~~
init
| x0 as $x | update
| ...
| xn as $x | update
~~~

and `foreach xs as $x (init; update; project)` evaluates to

~~~ text
init |
( x0 as $x | update | project,
( ...
( xn as $x | update | project,
( empty )...)
~~~

The interpretation of `reduce`/`foreach` in jaq has the following advantages over jq:

* It deals very naturally with filters that yield multiple outputs.
  In contrast, jq discriminates outputs of `f`,
  because it recurses only on the last of them,
  although it outputs all of them.
  <details><summary>Example</summary>

  `foreach (5, 10) as $x (1; .+$x, -.)` yields
  `6, -1, 9, 1` in jq, whereas it yields
  `6, 16, -6, -1, 9, 1` in jaq.
  We can see that both jq and jaq yield the values `6` and `-1`
  resulting from the first iteration (where `$x` is 5), namely
  `1 | 5 as $x | (.+$x, -.)`.
  However, jq performs the second iteration (where `$x` is 10)
  *only on the last value* returned from the first iteration, namely `-1`,
  yielding the values `9` and `1` resulting from
  `-1 | 10 as $x | (.+$x, -.)`.
  jaq yields these values too, but it also performs the second iteration
  on all other values returned from the first iteration, namely `6`,
  yielding the values `16` and `-6` that result from
  ` 6 | 10 as $x | (.+$x, -.)`.

  </details>
* It makes the implementation of `reduce` and `foreach`
  special cases of the same code, reducing the potential for bugs.


## Miscellaneous

* Slurping: When files are slurped in (via the `-s` / `--slurp` option),
  jq combines the inputs of all files into one single array, whereas
  jaq yields an array for every file.
  This is motivated by the `-i` / `--in-place` option,
  which could not work with the behaviour implemented by jq.
  The behaviour of jq can be approximated in jaq;
  for example, to achieve the output of
  `jq -s . a b`, you may use
  `jaq -s . <(cat a b)`.
* Cartesian products:
  In jq, `[(1,2) * (3,4)]` yields `[3, 6, 4, 8]`, whereas
  `[{a: (1,2), b: (3,4)} | .a * .b]` yields `[3, 4, 6, 8]`.
  jaq yields `[3, 4, 6, 8]` in both cases.
* Indexing `null`:
  In jq, when given `null` input, `.["a"]` and `.[0]` yield `null`, but `.[]` yields an error.
  jaq yields an error in all cases to prevent accidental indexing of `null` values.
  To obtain the same behaviour in jq and jaq, you can use
  `.["a"]? // null` or `.[0]? // null` instead.
* List updating:
  In jq, `[0, 1] | .[3] = 3` yields `[0, 1, null, 3]`; that is,
  jq fills up the list with `null`s if we update beyond its size.
  In contrast, jaq fails with an out-of-bounds error in such a case.
* Input reading:
  When there is no more input value left,
  in jq, `input` yields an error, whereas in jaq, it yields no output value.
* Joining:
  When giving an array `[x1, ..., xn]` to `join($sep)`, jaq returns
  `""` if the array is empty, otherwise `"\(x1)" + $sep + ... + $sep + "\(xn)"`;
  that is, it concatenates the string representations of the array values interspersed with `$sep`.
  Unlike jq, jaq does not map `null` values in the array to `""`,
  nor does it reject array or object values in the array.



# Contributing

Contributions to jaq are welcome.
Please make sure that after your change, `cargo test` runs successfully.



# Acknowledgements

[This project](https://nlnet.nl/project/jaq/) was funded through the
<a href="https://nlnet.nl/entrust">NGI0 Entrust</a> Fund, a fund established by
<a href="https://nlnet.nl">NLnet</a> with financial support from the
European Commission's <a href="https://ngi.eu">Next Generation Internet</a>
programme, under the aegis of <a href="https://commission.europa.eu/about-european-commission/departments-and-executive-agencies/communications-networks-content-and-technology_en">DG Communications Networks, Content and Technology</a> under grant agreement N<sup>o</sup> 101069594.

jaq has also profited from:

* [serde_json] to read and [colored_json] to output JSON,
* [chumsky] to parse and [ariadne] to pretty-print parse errors,
* [mimalloc] to boost the performance of memory allocation, and
* the Rust standard library, in particular its awesome [Iterator],
  which builds the rock-solid base of jaq's filter execution

[serde_json]: https://docs.rs/serde_json/
[colored_json]: https://docs.rs/colored_json/
[chumsky]: https://docs.rs/chumsky/
[ariadne]: https://docs.rs/ariadne/
[mimalloc]: https://docs.rs/mimalloc/
[Iterator]: https://doc.rust-lang.org/std/iter/trait.Iterator.html
