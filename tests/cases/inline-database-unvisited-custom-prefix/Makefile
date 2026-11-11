all:
	+@$(MAKE) -rR --no-print-directory -f commands.mk -p all > database.txt
	@awk '/^> @printf / { raw = 1 } raw && NF { print } raw && !NF { exit }' database.txt
	@test ! -e should-not-exist
	@rm -f database.txt
