# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

NAME=early
%.t:$(info name=$(NAME))$(NAME)=value
NAME=later
all:a.t b.t
a.t b.t:;@printf '%s=<%s>/<%s>\n' '$@' '$(early)' '$(later)'
