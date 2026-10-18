# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

NAME = global
%.task: NAME = local
%.task: $(NAME) = value
all: red.task
red.task:
	@printf '%s/%s/%s\n' '$(NAME)' '$(global)' '$(local)'
