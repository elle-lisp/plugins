(elle/epoch 15)

## egui plugin integration tests: a window left open at exit
##
## A window needs a display. CI has none, and elle's portable build does not
## build this plugin, so the file skips when either is missing. A process
## opens one window, so egui-close.lisp holds the window that egui/close
## closes.
##
## The file ends with the window still open, and its exit status is the last
## assertion. Elle frees the handle as the process exits. The trap: a handle
## that drops its event loop first disconnects Wayland while the clipboard
## thread still holds proxies on it. That thread then faults in
## wl_proxy_destroy, and the process exits 139 after every assertion passed.

(def [built? egui] (protect (import-file "target/release/libelle_egui.so")))
(unless built?
  (println "SKIP: egui plugin not built")
  (exit 0))

(def [shown? win] (protect (egui:open {:title "egui test: left open"})))
(unless shown?
  (println "SKIP: no display")
  (exit 0))

(def ix (egui:frame win [:label "left open"]))
(assert (= ix:closed false) "a window nobody closed is not :closed")
(assert (egui:open? win) "a window drawn once is open")

(println "egui: all tests passed")
