; injections.scm for the Op language

; Treat doc-comment text as Markdown for potential rendering by plugins
; that support injected language highlighting.
((doc_comment) @injection.content
  (#set! injection.language "markdown"))

((module_doc_comment) @injection.content
  (#set! injection.language "markdown"))