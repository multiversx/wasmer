#include <stdio.h>
#include "wasmer.h"
#include <assert.h>
#include <string.h>
wasmer_instance_t *instance;
// Function to print the most recent error string from Wasmer if we have them
void print_wasmer_error()
{
  int error_len = wasmer_last_error_length();
  char *error_str = malloc(error_len);
  wasmer_last_error_message(error_str, error_len);
  printf("Error: `%s`\n", error_str);
}

void interrupt_execution(wasmer_instance_context_t *ctx) {
  printf("Interrupting execution of the guest Wasm module, from the imported host function ...\n");
  wasmer_instance_set_runtime_breakpoint_value(instance, 2);
}

void should_not_be_called(wasmer_instance_context_t *ctx) {
  printf("calling second function...");
}

// Function to create a function import to pass to our wasmer instance
wasmer_import_func_t *create_wasmer_import_function(
    void (*function_pointer)(void *), // A Pointer to the host functiono
    wasmer_value_tag params_signature[],  // Function signature for the function params
    int num_params,  // Number of params
    wasmer_value_tag returns_signature[], // Function signature for the function returns
    int num_returns // Number of Returns
    ) {

  // Create a new func to hold the parameter and signature
  // of our `print_str` host function
  wasmer_import_func_t *func = wasmer_import_func_new(
      function_pointer,
      params_signature,
      num_params,
      returns_signature,
      num_returns
      );

  return func;
}

// Function to create a Wasmer Instance
wasmer_instance_t *create_wasmer_instance(
    wasmer_import_func_t *interrupt_execution_import_function,
    char *interrupt_execution_import_function_name,
    wasmer_import_func_t *should_not_be_called_import_function,
    char *should_not_be_called_import_function_name
    ) {

  // Create module name for our imports

  // Create a UTF-8 string as bytes for our module name.
  // And, place the string into the wasmer_byte_array type so it can be used by our guest Wasm instance.
  const char *module_name = "env";
  wasmer_byte_array module_name_bytes = { .bytes = (const uint8_t *) module_name,
    .bytes_len = strlen(module_name) };

  wasmer_byte_array interrupt_execution_import_function_name_bytes = { .bytes = (const uint8_t *) interrupt_execution_import_function_name,
    .bytes_len = strlen(interrupt_execution_import_function_name) };
  wasmer_import_t interrupt_execution_import = { .module_name = module_name_bytes,
    .import_name = interrupt_execution_import_function_name_bytes,
    .tag = WASM_FUNCTION,
    .value.func = interrupt_execution_import_function };

  wasmer_byte_array should_not_be_called_import_function_name_bytes = { .bytes = (const uint8_t *) should_not_be_called_import_function_name,
    .bytes_len = strlen(should_not_be_called_import_function_name) };
  wasmer_import_t should_not_be_called_import = { .module_name = module_name_bytes,
    .import_name = should_not_be_called_import_function_name_bytes,
    .tag = WASM_FUNCTION,
    .value.func = should_not_be_called_import_function };

  wasmer_import_t imports[] = {interrupt_execution_import, should_not_be_called_import};

  // Read the Wasm file bytes
  FILE *file = fopen("contracts/crash/output/crash.wasm", "r");
  assert(file != NULL);
  fseek(file, 0, SEEK_END);
  long len = ftell(file);
  uint8_t *bytes = malloc(len);
  fseek(file, 0, SEEK_SET);
  fread(bytes, 1, len, file);
  fclose(file);

  // Instantiate a WebAssembly Instance from Wasm bytes and imports
  wasmer_instance_t *instance = NULL;
  wasmer_result_t compile_result = wasmer_instantiate(
      &instance, // Our reference to our Wasm instance
      bytes, // The bytes of the WebAssembly modules
      len, // The length of the bytes of the WebAssembly module
      imports, // The Imports array the will be used as our importObject
      2 // The number of imports in the imports array
      );

  // Ensure the compilation was successful.
  if (compile_result != WASMER_OK)
  {
    print_wasmer_error();
  }

  // Assert the Wasm instantion completed
  assert(compile_result == WASMER_OK);

  // Return the Wasmer Instance
  return instance;
}

int main() {
  // Create the interrupt_execution function import
  wasmer_value_tag interrupt_execution_params_sig[] = {};
  wasmer_value_tag interrupt_execution_returns_sig[] = {};
  wasmer_import_func_t *interrupt_execution_import_function = create_wasmer_import_function(
      (void (*)(void *)) interrupt_execution, // Function Pointer
      interrupt_execution_params_sig, // Params Signature
      0, // Number of Params
      interrupt_execution_returns_sig, // Returns Signature
      0 // Number of Returns
      );

  // Create the should_not_be_called function import
  wasmer_value_tag should_not_be_called_params_sig[] = {};
  wasmer_value_tag should_not_be_called_returns_sig[] = {};
  wasmer_import_func_t *should_not_be_called_import_function = create_wasmer_import_function(
      (void (*)(void *)) should_not_be_called, // Function Pointer
      should_not_be_called_params_sig, // Params Signature
      0, // Number of Params
      should_not_be_called_returns_sig, // Returns Signature
      0 // Number of Returns
      );


  // Initialize our Wasmer Memory and Instance
  instance = create_wasmer_instance(
      interrupt_execution_import_function,
      "interrupt_execution",
      should_not_be_called_import_function,
      "should_not_be_called"
      );

  // Define our results. Results are created with { 0 } to avoid null issues,
  // And will be filled with the proper result after calling the guest Wasm function.
  wasmer_value_t result_one = { 0 };
  wasmer_value_t results[] = {result_one};

  // Define our parameters (none) we are passing into the guest Wasm function call.
  wasmer_value_t params[] = {0};

  // Call the Wasm function
  wasmer_result_t call_result = wasmer_instance_call(
      instance, // Our Wasm Instance
      "crashme", // the name of the exported function we want to call on the guest Wasm module
      params, // Our array of parameters
      0, // The number of parameters
      results, // Our array of results
      1 // The number of results
      );

  // Assert the call error'd (Because we error'd from the host import function)
  assert(call_result == WASMER_ERROR);
  print_wasmer_error();

  printf("Execution finished\n");

  // Destroy the instances we created for our wasmer
  wasmer_import_func_destroy(interrupt_execution_import_function);
  wasmer_import_func_destroy(should_not_be_called_import_function);
  wasmer_instance_destroy(instance);

  return 0;
}
