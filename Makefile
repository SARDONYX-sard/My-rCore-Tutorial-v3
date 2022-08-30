.PHONY: docker build_docker

docker:
	docker-compose up

build_docker:
	docker-compose build

FORMAT_COMMAND :=	cargo fmt
DIRS = $(sort $(dir $(wildcard ./*/Cargo.toml)))
IS_SUCCESS_PREV_CMD := bash -c "[ \"$$?\" == $"0$" ]"
DONE_MESSAGE := $(IS_SUCCESS_PREV_CMD) && printf "\033[0;32mDone.\033[0;0m\n"

fmt:
	@printf "\033[0;34mFormatting *.rs...\033[0;0m\n"
	@$(foreach dir, $(DIRS), cd "$(dir)" && $(FORMAT_COMMAND) && cd - >/dev/null;)
	@$(DONE_MESSAGE)

fmt-check:
	@printf "\033[0;34mcargo fmt --check *.rs...\033[0;0m\n"
	@$(foreach dir, $(DIRS), cd "$(dir)" && $(FORMAT_COMMAND) --all -- --check; cd - >/dev/null;)
	@printf "\033[0;32mDone.\033[0;0m\n"

check:
	@printf "\033[0;34mcargo check *.rs...\033[0;0m\n"
	@$(foreach dir, $(DIRS), cd "$(dir)" && cargo check && cd - >/dev/null;)
	@$(DONE_MESSAGE)

clean:
	@printf "\033[0;34mCleaning...\033[0;0m\n"
	@$(foreach dir, $(DIRS), cd "$(dir)" && cargo clean; cd - >/dev/null;)
	@$(DONE_MESSAGE)
