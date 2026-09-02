(elle/epoch 6)

## jiff plugin integration tests

## Try to load the jiff plugin. If it fails, exit cleanly.
(def [ok? plugin] (protect (import-file "target/release/libelle_jiff.so")))
(when (not ok?)
  (print "SKIP: jiff plugin not built\n")
  (exit 0))

(def date-fn          (get plugin :date))
(def time-fn          (get plugin :time))
(def weekday-fn       (get plugin :date/weekday))
(def next-weekday-fn  (get plugin :date/next-weekday))
(def prev-weekday-fn  (get plugin :date/prev-weekday))
(def year-fn          (get plugin :date/year))
(def month-fn         (get plugin :date/month))
(def day-fn           (get plugin :date/day))
(def round-fn         (get plugin :temporal/round))
(def hour-fn          (get plugin :time/hour))
(def minute-fn        (get plugin :time/minute))
(def second-fn        (get plugin :time/second))

## ── A keyword the plugin produces ──────────────────────────────
##
## A keyword is a bare name hash; the spelling lives in the memo of the
## instance that asked. The plugin reaches that memo through the per-call
## ctx, so these assertions are what fails when a primitive builds a
## keyword against the wrong ctx: the hash still compares equal to itself,
## but the name behind it is not the one this program wrote.

## 2024-06-19 was a Wednesday.
(assert (= (weekday-fn (date-fn 2024 6 19)) :wednesday)
        "date/weekday returns the keyword this program can name")

(assert (= (weekday-fn (date-fn 2024 6 17)) :monday)
        "date/weekday names Monday")

(assert (= (weekday-fn (date-fn 2024 6 23)) :sunday)
        "date/weekday names Sunday")

## The keyword the plugin minted prints as the spelling it was built from,
## which is the memo lookup rather than the equality compare.
(assert (= (string (weekday-fn (date-fn 2024 6 19))) "wednesday")
        "a plugin-minted keyword displays its own spelling")

## All seven, because a wrong ctx fails in a way that looks like working
## code. Elle carries a static vocabulary of spellings its own Rust mints,
## and :monday, :wednesday, :friday and :sunday are already in it. A
## primitive minting keywords against a null ctx therefore renders those
## four correctly and renders :tuesday, :thursday and :saturday as
## `#<keyword:0x…>`. Asserting one weekday passes on the broken build;
## asserting the week does not.
(def spellings
  (map (fn [d] (string (weekday-fn (date-fn 2024 6 d))))
       [17 18 19 20 21 22 23]))
(assert (= spellings
           ["monday" "tuesday" "wednesday" "thursday" "friday"
            "saturday" "sunday"])
        "all seven weekday keywords carry their spelling back")

## ── A keyword the plugin reads ─────────────────────────────────
##
## The other direction: the plugin resolves a keyword this program wrote
## back to its spelling, again through the call's ctx. Read against the
## wrong ctx, the spelling comes back empty and the primitive rejects its
## own argument — "expected keyword, got keyword".

(def next-mon (next-weekday-fn (date-fn 2024 6 19) :monday))
(assert (= (year-fn next-mon) 2024) "date/next-weekday keeps the year")
(assert (= (month-fn next-mon) 6) "date/next-weekday keeps the month")
(assert (= (day-fn next-mon) 24) "date/next-weekday reads :monday")

(def prev-fri (prev-weekday-fn (date-fn 2024 6 19) :friday))
(assert (= (day-fn prev-fri) 14) "date/prev-weekday reads :friday")

## A keyword read out of a struct field, which is the third name path.
(def rounded (round-fn (time-fn 15 22 45) {:unit :hour}))
(assert (= (hour-fn rounded) 15) "temporal/round reads :unit from a struct")
(assert (= (minute-fn rounded) 0) "rounding to :hour clears the minutes")
(assert (= (second-fn rounded) 0) "rounding to :hour clears the seconds")

(def rounded-min (round-fn (time-fn 15 22 45) {:unit :minute}))
(assert (= (minute-fn rounded-min) 23) "temporal/round reads :minute")
(assert (= (second-fn rounded-min) 0) "rounding to :minute clears the seconds")

## A keyword the plugin cannot resolve is an error, not a silent default.
(def [round-ok? _] (protect (round-fn (time-fn 15 22 45) {:unit :fortnight})))
(assert (not round-ok?) "an unknown unit keyword is an error")

## A round-trip: what the plugin names, the plugin reads.
(def wd (weekday-fn (date-fn 2024 6 19)))
(assert (= (day-fn (next-weekday-fn (date-fn 2024 6 19) wd)) 26)
        "a keyword the plugin minted is one the plugin can read back")

(print "jiff plugin tests passed\n")
