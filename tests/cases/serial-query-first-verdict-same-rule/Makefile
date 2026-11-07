query: ; @$(info evaluated-query):
bad: ; +@printf 'forced-failure\n'; exit 7
wrapper: query bad
after: ; +@printf 'visited-after\n'
