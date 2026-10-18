# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

NAME = GLOBAL
all: red blue
red: NAME = RED
blue: NAME = BLUE
red blue: $(NAME) = value
red blue:
	@printf '%s=%s/%s/%s\n' '$@' '$(RED)' '$(BLUE)' '$(GLOBAL)'
