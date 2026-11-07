branch:: ; @$(info evaluated-branch):
branch:: ; +@printf 'forced-failure\n'; exit 7
wrapper: branch
after: ; +@printf 'visited-after\n'
