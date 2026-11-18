(elle/epoch 15)

## Makefile tests
##
## `make space` reclaims disk the way elle's own target does: it deletes
## cargo's intermediate files from the directory the plugins build into,
## and keeps what was built. Run from the repository root, as CI runs
## every test.

(defn make [args]
  (subprocess/system "make" args))

(defn words [text]
  (let [found @[]]
    (each w in (string/split (string/replace text "\n" " ") " ")
      (when (not (empty? w)) (push found w)))
    (freeze found)))

(defn member? [x coll]
  (= true (each y in coll (when (= x y) (break true)))))

## ── the directory the build writes is the directory space prunes ──

(def build-root
  (if (path/exists? "../elle-plugin/Cargo.toml") "../target" "target"))

(def portable (make ["-n" "portable"]))
(assert (= (get portable :exit) 0) "a dry run of the portable build succeeds")
(def portable-words (words (get portable :stdout)))
(assert (member? "--target-dir" portable-words)
        "the portable build names its target directory")
(assert (member? build-root portable-words)
        "the portable build writes the submodule's parent target, or this repo's own")

(def dry (make ["-n" "space"]))
(assert (= (get dry :exit) 0) "a dry run of space succeeds")
(def dry-words (words (get dry :stdout)))
(each profile in ["debug" "release" "ci"]
  (assert (member? (string build-root "/" profile "/deps") dry-words)
          (string "space prunes " profile " in the directory the build writes")))

(def help (make ["-s" "help"]))
(assert (member? "space" (words (get help :stdout))) "help lists space")

## ── a pruned tree keeps what was built ─────────────────────────

(def profiles ["debug" "release" "ci"])
(def intermediates ["deps" "build" "incremental" ".fingerprint" "examples"])

(defn plant [root rel]
  (let [file (path/join root rel)]
    (file/mkdir-all (path/parent file))
    (file/write file "x")
    file))

(with-temp-dir root
  (each profile in profiles
    (each dir in intermediates
      (plant root (path/join profile dir "crate-0123" "artifact"))))
  (def kept
    (concat (map (fn [p] (plant root (path/join p "libelle_fake.so"))) profiles)
            (map (fn [p] (plant root (path/join p "libelle_fake.d"))) profiles)
            [(plant root "CACHEDIR.TAG")
             (plant root (path/join "doc" "index.html"))
             (plant root (path/join "profiling" "deps" "artifact"))]))

  (def ran (make ["-s" "space" (string "TARGET_ROOT=" root)]))
  (assert (= (get ran :exit) 0)
          (string "space succeeds on a populated tree: " (get ran :stderr)))

  (each profile in profiles
    (assert (path/dir? (path/join root profile))
            (string "the " profile " directory itself stays"))
    (each dir in intermediates
      (assert (not (path/exists? (path/join root profile dir)))
              (string "space deletes " profile "/" dir))))

  (each file in kept
    (assert (path/file? file) (string "space keeps " file)))

  (def again (make ["-s" "space" (string "TARGET_ROOT=" root)]))
  (assert (= (get again :exit) 0) "space succeeds on a tree it already pruned"))

(with-temp-dir root
  (def ran (make ["-s" "space" (string "TARGET_ROOT=" root)]))
  (assert (= (get ran :exit) 0) "space succeeds on a tree that was never built"))
