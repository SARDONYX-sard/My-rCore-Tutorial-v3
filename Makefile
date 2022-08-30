.PHONY: docker build_docker

docker:
	docker-compose up

build_docker:
	docker-compose build

FORMAT_COMMAND :=	cargo fmt
DIRS = $(sort $(dir $(wildcard ./*/Cargo.toml)))
IS_SUCCESS_PREV_CMD := bash -c "[ \"$$?\" == $"0$" ]"

ANSI_ESC = \033[
BLUE_COLOR = $(ANSI_ESC)34m
RESET_COLOR = $(ANSI_ESC)0m
DONE_MESSAGE := $(IS_SUCCESS_PREV_CMD) && printf "$(ANSI_ESC)32mDone.$(RESET_COLOR)\n"

fmt:
	@printf "\033[0;34mFormatting *.rs...\033[0;0m\n"
	@$(foreach dir, $(DIRS), cd "$(dir)" && $(FORMAT_COMMAND) && cd - >/dev/null;)
	@$(DONE_MESSAGE)

fmt-check:
	@printf "$(BLUE_COLOR)cargo fmt --check *.rs...$(RESET_COLOR)\n"
	@$(foreach dir, $(DIRS), cd "$(dir)" && $(FORMAT_COMMAND) --all -- --check; cd - >/dev/null;)
	@$(DONE_MESSAGE)

check:
	@printf "$(BLUE_COLOR)cargo check *.rs...$(RESET_COLOR)\n"
	@$(foreach dir, $(DIRS), cd "$(dir)" && cargo check && cd - >/dev/null;)
	@$(DONE_MESSAGE)

clean:
	@printf "$(BLUE_COLOR)Cleaning...$(RESET_COLOR)\n"
	@$(foreach dir, $(DIRS), cd "$(dir)" && cargo clean; cd - >/dev/null;)
	@$(DONE_MESSAGE)

clippy:
	$(foreach dir, $(DIRS), cd "$(dir)" && cargo clippy; cd - >/dev/null;)
	@$(DONE_MESSAGE)
