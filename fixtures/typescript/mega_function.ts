// Test fixture for ALP-922: nested symbol extraction from mega-functions

export function createTypeChecker(host: any): any {
  // Prologue vars — non-trivial (have call expressions or type annotations)
  var silentNeverType = createIntrinsicType(TypeFlags.Never, "never");
  const checker: TypeChecker = {} as TypeChecker;
  var compilerOptions = host.getCompilerOptions();

  // Trivial prologue vars — should NOT be extracted
  var inStrictMode = false;
  let counter = 0;

  // Depth-1 nested function declarations — should be extracted
  function getIndexType(type: any, index: any): any {
    return undefined;
  }

  function getReturnType(signature: any): any {
    return undefined;
  }

  // A nested function with its own nested function (depth > 1 — should NOT be extracted)
  function outerHelper() {
    function innerHelper() {
      // depth 2 — must not appear in index
    }
  }

  return checker;
}

// Non-exported function with nested declarations — should still be indexed
function internalHelper(): void {
  var state = createState();

  function processItem(item: any): void {
    return;
  }
}
