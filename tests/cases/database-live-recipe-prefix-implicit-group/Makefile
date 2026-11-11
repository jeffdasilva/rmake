.PHONY: scan
scan:
	+@$(MAKE) -rR --no-print-directory -f rules.mk -np all > database.txt
	@awk -f select.awk database.txt
	@rm database.txt
