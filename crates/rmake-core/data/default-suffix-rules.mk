# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 1988-2023 Free Software Foundation, Inc.
# Observed and organized for rmake in 2026 by Jeff DaSilva.
# Linux GNU Make 4.4.1 compatibility data observed with the pinned oracle.
# Recipe and variable bytes are declarative compatibility data, not C source.
.C:
	$(LINK.C) $^ $(LOADLIBES) $(LDLIBS) -o $@

.C.o:
	$(COMPILE.C) $(OUTPUT_OPTION) $<

.F:
	$(LINK.F) $^ $(LOADLIBES) $(LDLIBS) -o $@

.F.f:
	$(PREPROCESS.F) $(OUTPUT_OPTION) $<

.F.o:
	$(COMPILE.F) $(OUTPUT_OPTION) $<

.S:
	$(LINK.S) $^ $(LOADLIBES) $(LDLIBS) -o $@

.S.o:
	$(COMPILE.S) -o $@ $<

.S.s:
	$(PREPROCESS.S) $< > $@

.c:
	$(LINK.c) $^ $(LOADLIBES) $(LDLIBS) -o $@

.c.ln:
	$(LINT.c) -C$* $<

.c.o:
	$(COMPILE.c) $(OUTPUT_OPTION) $<

.cc:
	$(LINK.cc) $^ $(LOADLIBES) $(LDLIBS) -o $@

.cc.o:
	$(COMPILE.cc) $(OUTPUT_OPTION) $<

.cpp:
	$(LINK.cpp) $^ $(LOADLIBES) $(LDLIBS) -o $@

.cpp.o:
	$(COMPILE.cpp) $(OUTPUT_OPTION) $<

.def.sym:
	$(COMPILE.def) -o $@ $<

.f:
	$(LINK.f) $^ $(LOADLIBES) $(LDLIBS) -o $@

.f.o:
	$(COMPILE.f) $(OUTPUT_OPTION) $<

.l.c:
	@$(RM) $@ 
	 $(LEX.l) $< > $@

.l.ln:
	@$(RM) $*.c
	 $(LEX.l) $< > $*.c
	$(LINT.c) -i $*.c -o $@
	 $(RM) $*.c

.l.r:
	$(LEX.l) $< > $@ 
	 mv -f lex.yy.r $@

.lm.m:
	@$(RM) $@ 
	 $(LEX.m) $< > $@

.m:
	$(LINK.m) $^ $(LOADLIBES) $(LDLIBS) -o $@

.m.o:
	$(COMPILE.m) $(OUTPUT_OPTION) $<

.mod:
	$(COMPILE.mod) -o $@ -e $@ $^

.mod.o:
	$(COMPILE.mod) -o $@ $<

.o:
	$(LINK.o) $^ $(LOADLIBES) $(LDLIBS) -o $@

.p:
	$(LINK.p) $^ $(LOADLIBES) $(LDLIBS) -o $@

.p.o:
	$(COMPILE.p) $(OUTPUT_OPTION) $<

.r:
	$(LINK.r) $^ $(LOADLIBES) $(LDLIBS) -o $@

.r.f:
	$(PREPROCESS.r) $(OUTPUT_OPTION) $<

.r.o:
	$(COMPILE.r) $(OUTPUT_OPTION) $<

.s:
	$(LINK.s) $^ $(LOADLIBES) $(LDLIBS) -o $@

.s.o:
	$(COMPILE.s) -o $@ $<

.sh:
	cat $< >$@ 
	 chmod a+x $@

.tex.dvi:
	$(TEX) $<

.texi.dvi:
	$(TEXI2DVI) $(TEXI2DVI_FLAGS) $<

.texi.info:
	$(MAKEINFO) $(MAKEINFO_FLAGS) $< -o $@

.texinfo.dvi:
	$(TEXI2DVI) $(TEXI2DVI_FLAGS) $<

.texinfo.info:
	$(MAKEINFO) $(MAKEINFO_FLAGS) $< -o $@

.txinfo.dvi:
	$(TEXI2DVI) $(TEXI2DVI_FLAGS) $<

.txinfo.info:
	$(MAKEINFO) $(MAKEINFO_FLAGS) $< -o $@

.w.c:
	$(CTANGLE) $< - $@

.w.tex:
	$(CWEAVE) $< - $@

.web.p:
	$(TANGLE) $<

.web.tex:
	$(WEAVE) $<

.y.c:
	$(YACC.y) $< 
	 mv -f y.tab.c $@

.y.ln:
	$(YACC.y) $< 
	 $(LINT.c) -C$* y.tab.c 
	 $(RM) y.tab.c

.ym.m:
	$(YACC.m) $< 
	 mv -f y.tab.c $@
