.PHONY: verify
verify: ; @python3 observer.py '$(EXPECT_DIR)' '$(STYLE)'
