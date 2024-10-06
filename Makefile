# Variables
DOCKER_COMPOSE = docker-compose
SQLX = sqlx
CARGO = cargo
MYSQL_USER = root
MYSQL_PASSWORD = test
MYSQL_HOST = 127.0.0.1
MYSQL_PORT = 3306

.PHONY: all docker-up wait-for-mysql create-db migrate test bench build cargo-test cargo-bench cargo-build docker-clean clean

# Default target
all: migrate cargo-test cargo-bench cargo-build docker-clean

# Start Docker Compose
docker-up:
	$(DOCKER_COMPOSE) up -d

# Wait for MySQL to be ready
wait-for-mysql: docker-up
	@until docker compose exec -T mariadb mariadb-admin ping -h"$(MYSQL_HOST)" -P"$(MYSQL_PORT)" --user="$(MYSQL_USER)" --password="$(MYSQL_PASSWORD)" --silent &> /dev/null ; do \
		echo "MySQL is unavailable - sleeping"; \
		sleep 3; \
	done
	@echo "MySQL is up - continuing..."

# Create the database with sqlx
create-db: wait-for-mysql
	$(SQLX) database create

# Apply migrations with sqlx
migrate: create-db
	$(SQLX) migrate run

# Run tests with Cargo
cargo-test:
	$(CARGO) test -- --nocapture --test-threads=1

# Run test and clean
test: migrate cargo-test
	$(MAKE) docker-clean

# Run bench
cargo-bench:
	$(CARGO) bench

# Run bench and clean
bench: migrate cargo-bench
	$(MAKE) docker-clean

# Run build
cargo-build:
	$(CARGO) build --release

# Run build and clean
build: migrate cargo-build
	$(MAKE) docker-clean

# Clean up: stop and remove Docker containers (optional)
docker-clean:
	$(DOCKER_COMPOSE) down

# Clean all
clean:
	$(CARGO) clean
