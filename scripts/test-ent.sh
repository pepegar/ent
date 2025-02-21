#!/bin/bash
# Test commands for ENT CLI
# Usage: ./test-ent.sh

set -e  # Exit on error

# Function to wait for server to be ready
wait_for_server() {
    local retries=0
    local max_retries=30
    local server=$1

    echo "Waiting for server to be ready..."
    while ! grpcurl -plaintext $server list > /dev/null 2>&1; do
        ((retries++))
        if [ $retries -eq $max_retries ]; then
            echo "Server failed to start after $max_retries attempts"
            exit 1
        fi
        echo "Waiting for server... attempt $retries/$max_retries"
        sleep 1
    done

    # List available services
    echo "Available gRPC services:"
    grpcurl -plaintext $server list

    # Show service details
    echo -e "\nSchema Service details:"
    grpcurl -plaintext $server describe ent.SchemaService || echo "SchemaService not found"
    
    echo -e "\nGraph Service details:"
    grpcurl -plaintext $server describe ent.GraphService || echo "GraphService not found"

    echo "Server is ready!"
}

# Function to generate a JWT token
generate_jwt() {
    local user_id="test-user-123"
    local exp=$(($(date +%s) + 3600))  # 1 hour from now
    local header='{"alg":"RS256","typ":"JWT"}'
    local claims="{\"sub\":\"$user_id\",\"exp\":$exp,\"iss\":\"ent\"}"
    
    # Base64url encode header and claims
    local b64_header=$(echo -n "$header" | base64 | tr '+/' '-_' | tr -d '=')
    local b64_claims=$(echo -n "$claims" | base64 | tr '+/' '-_' | tr -d '=')
    
    # Create signature input
    local sig_input="$b64_header.$b64_claims"
    
    # Sign with private key
    local signature=$(echo -n "$sig_input" | openssl dgst -sha256 -sign test/data/private.pem | base64 | tr '+/' '-_' | tr -d '=')
    
    # Combine all parts
    echo "$sig_input.$signature"
}

# Function to run the CLI
run_cli() {
    local token=$(generate_jwt)
    RUST_LOG=debug cargo run --bin ent -- --endpoint "$ENDPOINT" --auth "Bearer $token" "$@"
}

# Check if required tools are installed
for cmd in grpcurl sqlx docker; do
    if ! command -v $cmd &> /dev/null; then
        echo "$cmd is required but not installed. Please install it first."
        exit 1
    fi
done

# Start Postgres using docker if not already running
if ! docker ps | grep -q "postgres:15-alpine"; then
    echo "Starting Postgres..."
    docker run -d \
        --name ent-postgres \
        -e POSTGRES_USER=ent \
        -e POSTGRES_PASSWORD=ent_password \
        -e POSTGRES_DB=ent \
        -p 5432:5432 \
        postgres:15-alpine
    
    # Wait for Postgres to be ready
    echo "Waiting for Postgres to be ready..."
    until docker exec ent-postgres pg_isready -U ent; do
        sleep 1
    done
fi

# Set the server address
SERVER="localhost:50051"
ENDPOINT="http2://localhost:50051"

# Export environment variables for the server
export ENT_DATABASE_URL="postgres://ent:ent_password@localhost:5432/ent"
export ENT_SERVER_HOST="0.0.0.0"
export ENT_SERVER_PORT="50051"
export ENT_JWT_PUBLIC_KEY_PATH="./test/data/public.pem"
export ENT_JWT_ISSUER="ent"
export RUST_LOG=debug

# Create test JWT keys if they don't exist
mkdir -p test/data
if [ ! -f test/data/private.pem ] || [ ! -f test/data/public.pem ]; then
    echo "Generating test JWT keys..."
    openssl genpkey -algorithm RSA -out test/data/private.pem
    openssl rsa -pubout -in test/data/private.pem -out test/data/public.pem
fi

# Run database migrations
echo "Running database migrations..."
DATABASE_URL=$ENT_DATABASE_URL sqlx database create
DATABASE_URL=$ENT_DATABASE_URL sqlx migrate run

# Start the server in the background
echo "Starting server..."
RUST_LOG=debug cargo run --bin ent-server &
SERVER_PID=$!

# Ensure we kill the server on script exit
trap 'kill $SERVER_PID 2>/dev/null' EXIT

# Wait for the server to be ready
wait_for_server $SERVER

# Generate unique type names using timestamp
TIMESTAMP=$(date +%s)
PERSON_TYPE="person_${TIMESTAMP}"
INVALID_TYPE="invalid_${TIMESTAMP}"

# Function to run a test case and handle its output
run_test_case() {
    local test_name=$1
    local expected_result=$2
    shift 2
    
    echo -e "\n=== Test Case: $test_name ==="
    echo "Command: ent $@"
    
    if run_cli "$@"; then
        if [ "$expected_result" = "success" ]; then
            echo "✅ Test passed (expected success)"
        else
            echo "❌ Test failed (expected failure but got success)"
            return 1
        fi
    else
        if [ "$expected_result" = "failure" ]; then
            echo "✅ Test passed (expected failure)"
        else
            echo "❌ Test failed (expected success but got failure)"
            return 1
        fi
    fi
}

echo "Testing Schema Service..."
echo "========================"

# Test Case 1: Valid simple schema
echo '{
  "type": "object",
  "properties": {
    "name": {"type": "string"},
    "age": {"type": "number"}
  }
}' > /tmp/valid-schema.json
run_test_case "Simple Schema" "success" \
    admin create-schema --file /tmp/valid-schema.json \
    --type-name "$PERSON_TYPE" --description "Person schema"

# Test Case 2: Invalid JSON schema
echo "invalid json" > /tmp/invalid-schema.json
run_test_case "Invalid JSON" "failure" \
    admin create-schema --file /tmp/invalid-schema.json \
    --type-name "$INVALID_TYPE"

# Test Case 3: Nested object schema
echo '{
  "type": "object",
  "properties": {
    "user": {
      "type": "object",
      "properties": {
        "name": {"type": "string"},
        "address": {
          "type": "object",
          "properties": {
            "street": {"type": "string"},
            "city": {"type": "string"}
          }
        }
      }
    }
  }
}' > /tmp/nested-schema.json
run_test_case "Nested Objects" "success" \
    admin create-schema --file /tmp/nested-schema.json \
    --type-name "nested_${TIMESTAMP}" --description "Schema with nested objects"

# Test Case 4: Array type schema
echo '{
  "type": "object",
  "properties": {
    "tags": {
      "type": "array",
      "items": {"type": "string"}
    }
  }
}' > /tmp/array-schema.json
run_test_case "Array Type" "success" \
    admin create-schema --file /tmp/array-schema.json \
    --type-name "array_${TIMESTAMP}" --description "Schema with array type"

# Test Case 5: Special characters in type name
run_test_case "Special Characters" "failure" \
    admin create-schema --file /tmp/valid-schema.json \
    --type-name "special@${TIMESTAMP}" --description "Schema with special chars"

# Test Case 6: Empty description
run_test_case "Empty Description" "success" \
    admin create-schema --file /tmp/valid-schema.json \
    --type-name "empty_desc_${TIMESTAMP}"

# Test Case 7: Duplicate type name
run_test_case "Duplicate Type" "failure" \
    admin create-schema --file /tmp/valid-schema.json \
    --type-name "$PERSON_TYPE" --description "Duplicate type name test"

# Test Case 8: Required fields
echo '{
  "type": "object",
  "required": ["email"],
  "properties": {
    "email": {"type": "string", "format": "email"},
    "name": {"type": "string"}
  }
}' > /tmp/required-schema.json
run_test_case "Required Fields" "success" \
    admin create-schema --file /tmp/required-schema.json \
    --type-name "required_${TIMESTAMP}" --description "Schema with required fields"

# Additional type name validation test cases
echo '{
  "type": "object",
  "properties": {
    "name": {"type": "string"}
  }
}' > /tmp/simple-schema.json

# Test Case 9: Type name starting with number (should fail)
run_test_case "Type Name Starting with Number" "failure" \
    admin create-schema --file /tmp/simple-schema.json \
    --type-name "1invalid_${TIMESTAMP}" --description "Invalid type name starting with number"

# Test Case 10: Type name with hyphens (should fail)
run_test_case "Type Name with Hyphens" "failure" \
    admin create-schema --file /tmp/simple-schema.json \
    --type-name "invalid-name-${TIMESTAMP}" --description "Invalid type name with hyphens"

# Test Case 11: Type name with spaces (should fail)
run_test_case "Type Name with Spaces" "failure" \
    admin create-schema --file /tmp/simple-schema.json \
    --type-name "invalid name ${TIMESTAMP}" --description "Invalid type name with spaces"

# Test Case 12: Valid type name with underscores and numbers
run_test_case "Valid Complex Type Name" "success" \
    admin create-schema --file /tmp/simple-schema.json \
    --type-name "valid_name_123_${TIMESTAMP}" --description "Valid type name with underscores and numbers"

# Test Case 13: Empty type name (should fail)
run_test_case "Empty Type Name" "failure" \
    admin create-schema --file /tmp/simple-schema.json \
    --type-name "" --description "Empty type name"

# Cleanup test files
rm -f /tmp/valid-schema.json /tmp/invalid-schema.json \
      /tmp/nested-schema.json /tmp/array-schema.json \
      /tmp/required-schema.json /tmp/simple-schema.json

echo -e "\nTesting Graph Service..."
echo "------------------------"

# Create test objects
echo "1. Creating test objects..."
echo '{
  "type": "post",
  "metadata": {
    "title": "Test Post",
    "content": "This is a test post"
  }
}' > /tmp/post-object.json
run_test_case "Create Post Object" "success" \
    create-object --file /tmp/post-object.json --type post

echo '{
  "type": "user",
  "metadata": {
    "name": "Test User",
    "email": "test@example.com"
  }
}' > /tmp/user-object.json
run_test_case "Create User Object" "success" \
    create-object --file /tmp/user-object.json --type user

# Test object retrieval
echo "2. Testing object retrieval..."
run_test_case "Get Post Object" "success" \
    get-object --object-id 1 --consistency full

run_test_case "Get User Object" "success" \
    get-object --object-id 2 --consistency minimum

# Create edges between objects
echo "3. Creating edges between objects..."
run_test_case "Create Author Edge" "success" \
    create-edge \
    --from-id 1 --from-type post \
    --to-id 2 --to-type user \
    --relation author

run_test_case "Create Comment Edge" "success" \
    create-edge \
    --from-id 1 --from-type post \
    --to-id 2 --to-type user \
    --relation comment

# Test edge retrieval
echo "4. Testing edge retrieval..."
run_test_case "Get Author Edge" "success" \
    get-edge --object-id 1 --edge-type author

run_test_case "Get Comment Edge" "success" \
    get-edge --object-id 1 --edge-type comment

run_test_case "Get All Edges" "success" \
    get-edges --object-id 1 --edge-type comment

# Cleanup test files
rm -f /tmp/post-object.json /tmp/user-object.json

# The server will be killed by the trap on exit