(elle/epoch 15)

## egui plugin integration tests: a window closed by egui/close
##
## A window needs a display. CI has none, and elle's portable build does not
## build this plugin, so the file skips when either is missing. A process
## opens one window, so egui.lisp holds the window left open at exit.
##
## Elle frees the closed window's handle as the process exits, so the handle
## releases its resources a second time. The second release must find nothing
## left to release, and the file's exit status is that assertion.

(def [built? egui] (protect (import-file "target/release/libelle_egui.so")))
(unless built?
  (println "SKIP: egui plugin not built")
  (exit 0))

(def [shown? win] (protect (egui:open {:title "egui test: closed"})))
(unless shown?
  (println "SKIP: no display")
  (exit 0))

(egui:frame win [:label "closed"])
(assert (egui:open? win) "a window is open before egui/close")
(egui:close win)
(assert (not (egui:open? win)) "a window is not open after egui/close")

(let [[ok? err] (protect (egui:open {:title "egui test: second"}))]
  (assert (not ok?) "a process opens one window, even after egui/close")
  (assert (= (get err :error) :egui-error)
          (concat "a second window is an :egui-error, got " (string err))))

(println "egui-close: all tests passed")
