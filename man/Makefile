#!/usr/bin/make -f
# (c) 2016 Christoph Grenz <christophg@grenz-bonn.de>
# License: GNU GPLv3 or later
# This file is part of tor6tun

PREFIX ?= /usr/local
SRCPATH := $(shell dirname $(lastword $(MAKEFILE_LIST)))
VPATH := $(SRCPATH)

MANPAGES = gunzip-split.1

all: $(MANPAGES:=.gz)

$(MANPAGES): %: %.md Makefile
	RONN_MANUAL="gunzip-split" ronn --pipe -r "$<" > "$@"

%.gz: %
	gzip -9 -k "$<" -c > "$@"

clean:
	rm $(MANPAGES:=.gz) || true

distclean: clean
	rm $(MANPAGES) || true

install: all
	install -m 0755 -o root -g root -d "$(PREFIX)"/share/man/man1/
	install -m 0755 -o root -g root gunzip-split.1.gz "$(PREFIX)"/share/man/man1/

uninstall:
	rm "$(PREFIX)"/share/man/man1/gunzip-split.1.gz || true

.PHONY: all clean install uninstall

