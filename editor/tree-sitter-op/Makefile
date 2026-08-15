.PHONY: all generate build install test clean

TREE_SITTER ?= tree-sitter
CC ?= cc
NVIM_SITE ?= $(HOME)/.local/share/nvim/site

OP_PARSER := op.so
PARSER_SRC := src/parser.c
QUERY_DIR := queries/op
INSTALL_PARSER := $(NVIM_SITE)/parser/op.so
INSTALL_QUERIES := $(NVIM_SITE)/queries/op

all: generate build install

generate:
	$(TREE_SITTER) generate

build: $(PARSER_SRC)
	$(TREE_SITTER) build --output $(OP_PARSER)

install: build
	install -d $(dir $(INSTALL_PARSER))
	install -d $(INSTALL_QUERIES)
	install -m 0644 $(OP_PARSER) $(INSTALL_PARSER)
	install -m 0644 $(QUERY_DIR)/*.scm $(INSTALL_QUERIES)/

test:
	$(TREE_SITTER) parse test/corpus/*.op

clean:
	rm -f $(OP_PARSER)
	rm -f src/parser.c src/grammar.json src/node-types.json
	rm -rf src/tree_sitter