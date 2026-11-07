query: ; @$(info evaluated-query):
bad: ; +@$(eval override MAKEFLAGS += -S)printf 'forced-failure\n'; exit 7
after: ; +@printf 'visited-after\n'
