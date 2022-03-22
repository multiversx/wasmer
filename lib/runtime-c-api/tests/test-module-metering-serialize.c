#include <stdio.h>
#include "../wasmer.h"
#include <assert.h>
#include <stdint.h>

int main()
{
    // Read the wasm file bytes
    FILE *file = fopen("assets/sum.wasm", "r");
    fseek(file, 0, SEEK_END);
    long len = ftell(file);
    uint8_t *bytes = malloc(len);
    fseek(file, 0, SEEK_SET);
    fread(bytes, 1, len, file);
    fclose(file);

    wasmer_module_t *module_one = NULL;
    unsigned long long gas_limit = 100;
    wasmer_result_t compile_result = wasmer_compile_with_gas_metering(&module_one, bytes, len);
    printf("Compile result: %d\n", compile_result);
    assert(compile_result == WASMER_OK);

	// first run before serialization
    wasmer_import_t imports[] = {};
    wasmer_instance_t *instance_one = NULL;
    wasmer_result_t instantiate_result = wasmer_module_instantiate(module_one, &instance_one, imports, 0);
    printf("Instantiate result: %d\n", instantiate_result);
    assert(instantiate_result == WASMER_OK);

	// check behavior of getting/setting points, also no error on null
    assert(wasmer_instance_get_points_used(instance_one) == 0);
    wasmer_instance_set_points_used(instance_one, 50);
    assert(wasmer_instance_get_points_used(instance_one) == 50);
    assert(wasmer_instance_get_points_used(NULL) == 0);

    wasmer_value_t param_one;
    param_one.tag = WASM_I32;
    param_one.value.I32 = 7;
    wasmer_value_t param_two;
    param_two.tag = WASM_I32;
    param_two.value.I32 = 8;
    wasmer_value_t params[] = {param_one, param_two};

    wasmer_value_t result_one;
    wasmer_value_t results[] = {result_one};

    wasmer_result_t call_result = wasmer_instance_call(instance_one, "sum", params, 2, results, 1);
    printf("Call result:  %d\n", call_result);
    printf("Result: %d\n", results[0].value.I32);
    assert(results[0].value.I32 == 15);
    assert(call_result == WASMER_OK);

	// ensure we got charged some gas
    assert(wasmer_instance_get_points_used(instance_one) == 54);
    // TODO: try again an ensure limit enforced... need another function

	// end first run

    wasmer_serialized_module_t *serialized_module = NULL;
    wasmer_result_t serialize_result = wasmer_module_serialize(&serialized_module, module_one);
    printf("Serialize result: %d\n", serialize_result);
    assert(serialize_result == WASMER_OK);

    wasmer_byte_array serialized_module_bytes = wasmer_serialized_module_bytes(serialized_module);
    printf("Serialized module pointer: %p\n", serialized_module_bytes.bytes);
    printf("Serialized module length: %d\n", serialized_module_bytes.bytes_len);
    assert(serialized_module_bytes.bytes != NULL);
    assert(serialized_module_bytes.bytes_len > 8);
    assert(serialized_module_bytes.bytes[0] == 'W');
    assert(serialized_module_bytes.bytes[1] == 'A');
    assert(serialized_module_bytes.bytes[2] == 'S');
    assert(serialized_module_bytes.bytes[3] == 'M');
    assert(serialized_module_bytes.bytes[4] == 'E');
    assert(serialized_module_bytes.bytes[5] == 'R');

    wasmer_module_t *module_two = NULL;
    wasmer_result_t unserialize_result = wasmer_module_deserialize(&module_two, serialized_module);
    assert(unserialize_result == WASMER_OK);

	// second run with deserialized module
    wasmer_instance_t *instance_two = NULL;
    instantiate_result = wasmer_module_instantiate(module_two, &instance_two, imports, 0);
    printf("Instantiate result: %d\n", instantiate_result);
    assert(instantiate_result == WASMER_OK);

	// ensure points independent of other instance
    assert(wasmer_instance_get_points_used(instance_one) > 50);
    assert(wasmer_instance_get_points_used(instance_two) == 0);
    wasmer_instance_set_points_used(instance_two, 20);
    assert(wasmer_instance_get_points_used(instance_one) > 50);
    assert(wasmer_instance_get_points_used(instance_two) == 20);

    call_result = wasmer_instance_call(instance_two, "sum", params, 2, results, 1);
    printf("Call result:  %d\n", call_result);
    printf("Result: %d\n", results[0].value.I32);
    assert(results[0].value.I32 == 15);
    assert(call_result == WASMER_OK);

	// and we charge the right one
	assert(wasmer_instance_get_points_used(instance_two) == 24);

    wasmer_serialized_module_t *serialized_module_two = NULL;
    wasmer_result_t serialized_module_from_bytes_result = wasmer_serialized_module_from_bytes(
        &serialized_module_two,
        serialized_module_bytes.bytes,
        serialized_module_bytes.bytes_len
    );
    assert(serialized_module_from_bytes_result == WASMER_OK);

    wasmer_module_t *module_three = NULL;
    wasmer_result_t unserialized_result_two = wasmer_module_deserialize(&module_three, serialized_module_two);
    assert(unserialized_result_two == WASMER_OK);

    wasmer_instance_t *instance_three = NULL;
    wasmer_result_t instantiate_result_two = wasmer_module_instantiate(module_three, &instance_three, imports, 0);
    assert(instantiate_result_two == WASMER_OK);

    printf("Destroy the serialized module\n");
    wasmer_serialized_module_destroy(serialized_module);
    wasmer_serialized_module_destroy(serialized_module_two);

    printf("Destroy instance\n");
    wasmer_instance_destroy(instance_one);
    wasmer_instance_destroy(instance_two);
    wasmer_instance_destroy(instance_three);

    printf("Destroy modules\n");
    wasmer_module_destroy(module_one);
    wasmer_module_destroy(module_two);
    return 0;
}
