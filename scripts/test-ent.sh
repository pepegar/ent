#!/bin/bash
# Test commands for ENT gRPC services
# Usage: ./test-ent.sh
# Note: Requires grpcurl to be installed

# Set the server address
SERVER="localhost:50051"

echo "Testing Schema Service..."
echo "------------------------"

# Create a valid schema
echo "1. Creating a valid schema..."
grpcurl -d '{
  "schema": "{\"type\": \"object\", \"properties\": {\"name\": {\"type\": \"string\"}, \"age\": {\"type\": \"number\"}}}"
}' \
-plaintext $SERVER ent.SchemaService/CreateSchema

# Create an invalid schema
echo -e "\n2. Testing invalid schema (should fail)..."
grpcurl -d '{
  "schema": "invalid json"
}' \
-plaintext $SERVER ent.SchemaService/CreateSchema

echo -e "\nTesting Graph Service..."
echo "------------------------"

# Get an object
echo "3. Getting an object..."
grpcurl -d '{
  "objectId": "1",
  "userToken": "test-token"
}' \
-plaintext $SERVER ent.GraphService/GetObject

# Get a single edge
echo -e "\n4. Getting a single edge..."
grpcurl -d '{
  "objectId": "1",
  "userToken": "test-token",
  "edge": "author"
}' \
-plaintext $SERVER ent.GraphService/GetEdge

# Get multiple edges
echo -e "\n5. Getting multiple edges..."
grpcurl -d '{
  "objectId": "1",
  "userToken": "test-token",
  "edge": "comments"
}' \
-plaintext $SERVER ent.GraphService/GetEdges

# Error cases

# Get non-existent object
echo -e "\n6. Getting non-existent object (should fail)..."
grpcurl -d '{
  "objectId": "999999",
  "userToken": "test-token"
}' \
-plaintext $SERVER ent.GraphService/GetObject

# Get edge with invalid object ID
echo -e "\n7. Getting edge with invalid object ID (should fail)..."
grpcurl -d '{
  "objectId": "invalid",
  "userToken": "test-token",
  "edge": "author"
}' \
-plaintext $SERVER ent.GraphService/GetEdge

# List available services and methods
echo -e "\nAvailable Services:"
echo "-------------------"
grpcurl -plaintext $SERVER list

# Show service details
echo -e "\nService Details:"
echo "----------------"
grpcurl -plaintext $SERVER list ent.GraphService
grpcurl -plaintext $SERVER list ent.SchemaService