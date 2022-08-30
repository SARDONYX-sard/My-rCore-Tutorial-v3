.PHONY: docker build_docker

docker:
	docker-compose up

build_docker:
	docker-compose build

FORMAT_COMMAND :=	cargo fmt
DIRS = $(sort $(dir $(wildcard ./*/Cargo.toml)))
fmt:
	@printf "\033[0;34mFormatting *.rs...\033[0;0m\n"
	@$(foreach dir, $(DIRS), cd "$(dir)" && $(FORMAT_COMMAND) && cd - >/dev/null;)
	@printf "\033[0;32mDone.\033[0;0m\n"

fmt-check:
	@printf "\033[0;34mChecking *.rs...\033[0;0m\n"
	@$(foreach dir, $(DIRS), cd "$(dir)" && $(FORMAT_COMMAND) --all -- --check && cd - >/dev/null;)
	@printf "\033[0;32mDone.\033[0;0m\n"
