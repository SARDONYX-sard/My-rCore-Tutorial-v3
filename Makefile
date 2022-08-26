.PHONY: docker build_docker

docker:
	docker-compose up

build_docker:
	docker-compose build

fmt:
	cd easy-fs; cargo fmt; cd ../easy-fs-fuse cargo fmt; cd ../os ; cargo fmt; cd ../user; cargo fmt; cd ..
