#!/usr/bin/env elle

## Test suite for plotters plugin

(elle/epoch 1)

(def [ok? plt] (protect (import "plugin/plotters")))
(when (not ok?)
  (println "SKIP: plotters plugin not built")
  (exit 0))

# ── Line chart ────────────────────────────────────────────────────────

(def line-png (plt:line [[1 20] [2 22] [3 19] [4 25] [5 28]]))
(assert (> (length line-png) 0) "line chart produces bytes")

## With options
(def line-opts (plt:line [[1 10] [2 20] [3 15]]
                         {:title "Test" :x-label "X" :y-label "Y"
                          :width 400 :height 300}))
(assert (> (length line-opts) 0) "line chart with opts")

## SVG output
(def line-svg (plt:line [[1 10] [2 20]] {:format :svg}))
(assert (string? line-svg) "line SVG returns string")
(assert (> (length line-svg) 0) "line SVG non-empty")

## Color option
(def line-red (plt:line [[1 10] [2 20]] {:color :red :width 200 :height 150}))
(assert (> (length line-red) 0) "line with color keyword")

(def line-rgb (plt:line [[1 10] [2 20]] {:color [0 128 255] :width 200 :height 150}))
(assert (> (length line-rgb) 0) "line with [r g b] color")

# ── Scatter plot ──────────────────────────────────────────────────────

(def scatter-png (plt:scatter [[1 20] [2 22] [3 19]]))
(assert (> (length scatter-png) 0) "scatter chart produces bytes")

(def scatter-svg (plt:scatter [[1 5] [2 10]] {:format :svg}))
(assert (string? scatter-svg) "scatter SVG returns string")

# ── Area chart ────────────────────────────────────────────────────────

(def area-png (plt:area [[1 20] [2 22] [3 19] [4 25]]))
(assert (> (length area-png) 0) "area chart produces bytes")

# ── Bar chart ─────────────────────────────────────────────────────────

(def bar-png (plt:bar ["Mon" "Tue" "Wed" "Thu" "Fri"] [10 20 15 25 18]))
(assert (> (length bar-png) 0) "bar chart produces bytes")

(def bar-opts (plt:bar ["A" "B"] [5 10]
                       {:title "Sales" :y-label "Count" :color :green}))
(assert (> (length bar-opts) 0) "bar chart with opts")

## Labels/values length mismatch → error
(def [bar-ok? _] (protect (plt:bar ["A" "B"] [1 2 3])))
(assert (not bar-ok?) "bar rejects mismatched lengths")

# ── Histogram ─────────────────────────────────────────────────────────

(def hist-png (plt:histogram [1.2 3.4 2.1 4.5 3.3 2.8 1.5 4.1 2.9 3.7]))
(assert (> (length hist-png) 0) "histogram produces bytes")

(def hist-bins (plt:histogram [1.0 2.0 3.0 4.0 5.0] {:bins 5}))
(assert (> (length hist-bins) 0) "histogram with bins option")

## Single value (edge case)
(def hist-one (plt:histogram [42.0]))
(assert (> (length hist-one) 0) "histogram with single value")

# ── Multi-series chart ────────────────────────────────────────────────

(def multi (plt:chart {:title "Multi"
                       :x-label "X" :y-label "Y"
                       :series [{:type :line :label "Line A"
                                 :data [[1 10] [2 20] [3 15]]}
                                {:type :scatter :label "Points"
                                 :data [[1 15] [2 18] [3 22]]}]}))
(assert (> (length multi) 0) "multi-series chart produces bytes")

## Three-series with colors
(def tri (plt:chart {:series [{:type :line :data [[0 0] [1 1]] :color :red}
                              {:type :line :data [[0 1] [1 0]] :color :blue}
                              {:type :area :data [[0 0.5] [1 0.5]] :color :green}]}))
(assert (> (length tri) 0) "three-series chart with explicit colors")

## SVG multi-series
(def multi-svg (plt:chart {:format :svg
                           :series [{:type :line :data [[1 1] [2 2]]}]}))
(assert (string? multi-svg) "multi-series SVG returns string")

# ── Type errors ───────────────────────────────────────────────────────

(def [ok1? _] (protect (plt:line "not an array")))
(assert (not ok1?) "line rejects non-array data")

(def [ok2? _] (protect (plt:chart {:series "not an array"})))
(assert (not ok2?) "chart rejects non-array series")

(def [ok3? _] (protect (plt:chart {:series [{:type :invalid :data [[1 2]]}]})))
(assert (not ok3?) "chart rejects invalid series type")

(println "plotters: all tests passed")
