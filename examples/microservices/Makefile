ROOT := $(abspath ../../)
CARGO ?= cargo
SERVICES := micro-gateway micro-sqlx-products micro-sqlx-auth micro-sqlx-users micro-sqlx-orders micro-sqlx-carts

.PHONY: help env build run-all stop-all logs ping \
	run-gateway run-products run-auth run-users run-orders run-carts

help:
	@echo "Ferrite microservices example"
	@echo ""
	@echo "  make env            copy .env.example -> .env for every service (first run)"
	@echo "  make build          build the whole workspace (all 6 services + crates)"
	@echo "  make run-all        start all 6 services in background (see .run-logs/)"
	@echo "  make stop-all       stop every background service"
	@echo "  make logs           tail all background service logs"
	@echo "  make ping           curl /healthz on every service"
	@echo ""
	@echo "  make run-gateway    run micro-gateway   (http :8080) in foreground"
	@echo "  make run-products   run micro-sqlx-products (http :3004, grpc :50051) in foreground"
	@echo "  make run-auth       run micro-sqlx-auth (http :3005, grpc :50052) in foreground"
	@echo "  make run-users      run micro-sqlx-users (http :3006, grpc :50053) in foreground"
	@echo "  make run-orders     run micro-sqlx-orders (http :3007, grpc :50054) in foreground"
	@echo "  make run-carts      run micro-sqlx-carts (http :3008, grpc :50055) in foreground"
	@echo ""
	@echo "Ports: gateway=8080 products=3004 auth=3005 users=3006 orders=3007 carts=3008"
	@echo "gRPC:  products=50051 auth=50052 users=50053 orders=50054 carts=50055"

env:
	@for svc in $(SERVICES); do \
		if [ -f "$$svc/.env" ]; then \
			echo "skip  $$svc/.env (exists)"; \
		else \
			cp "$$svc/.env.example" "$$svc/.env"; \
			echo "create $$svc/.env"; \
		fi; \
	done

build:
	$(CARGO) build --manifest-path $(ROOT)/Cargo.toml --workspace

run-all:
	@bash ./serve_all.sh

stop-all:
	@bash ./stop_all.sh

logs:
	@tail -n 20 .run-logs/*.log 2>/dev/null || echo "no logs yet (run 'make run-all' first)"

ping:
	@printf "gateway : "; curl -s -o /dev/null -w "%{http_code}\n" http://localhost:8080/healthz 2>/dev/null || true
	@printf "products: "; curl -s -o /dev/null -w "%{http_code}\n" http://localhost:3004/healthz 2>/dev/null || true
	@printf "auth    : "; curl -s -o /dev/null -w "%{http_code}\n" http://localhost:3005/healthz 2>/dev/null || true
	@printf "users   : "; curl -s -o /dev/null -w "%{http_code}\n" http://localhost:3006/healthz 2>/dev/null || true
	@printf "orders  : "; curl -s -o /dev/null -w "%{http_code}\n" http://localhost:3007/healthz 2>/dev/null || true
	@printf "carts   : "; curl -s -o /dev/null -w "%{http_code}\n" http://localhost:3008/healthz 2>/dev/null || true

run-gateway: env
	cd $(ROOT)/examples/microservices/micro-gateway && $(CARGO) run --manifest-path $(ROOT)/Cargo.toml -p micro-gateway

run-products: env
	cd $(ROOT)/examples/microservices/micro-sqlx-products && $(CARGO) run --manifest-path $(ROOT)/Cargo.toml -p micro-sqlx-products

run-auth: env
	cd $(ROOT)/examples/microservices/micro-sqlx-auth && $(CARGO) run --manifest-path $(ROOT)/Cargo.toml -p micro-sqlx-auth

run-users: env
	cd $(ROOT)/examples/microservices/micro-sqlx-users && $(CARGO) run --manifest-path $(ROOT)/Cargo.toml -p micro-sqlx-users

run-orders: env
	cd $(ROOT)/examples/microservices/micro-sqlx-orders && $(CARGO) run --manifest-path $(ROOT)/Cargo.toml -p micro-sqlx-orders

run-carts: env
	cd $(ROOT)/examples/microservices/micro-sqlx-carts && $(CARGO) run --manifest-path $(ROOT)/Cargo.toml -p micro-sqlx-carts
