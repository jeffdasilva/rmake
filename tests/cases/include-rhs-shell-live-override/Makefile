# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

X := old
X != $(eval override X:=new)printf ran >shell-ran; printf tail
$(info X=[$(X)] O=[$(origin X)] F=[$(flavor X)] R=[$(file <shell-ran)])
all:;@:
