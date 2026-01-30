# Testing

This project has test infrastructure set up for both frontend (TypeScript/Svelte) and backend (Rust).

## Running Tests

```bash
# Run all tests (frontend + backend)
npm run test:all

# Run only frontend tests
npm test

# Run frontend tests in watch mode
npm run test:watch

# Run only backend tests
cd src-tauri && cargo test
```

## Shared Fixtures (Frontend)

Reusable fixtures live under `src/tests/fixtures`:
- `license.ts` for license states
- `startup.ts` for startup info payloads
- `history.ts` for session history payloads

## Frontend Testing

- **Framework**: Vitest with @testing-library/svelte
- **Environment**: happy-dom
- **Location**: Tests are co-located with source files (`*.test.ts` files)

### Example: Store Tests

See `src/lib/stores/chat.test.ts` for examples of testing Svelte stores.

### Component Testing

Component testing with Svelte 5 is configured but may require additional setup for full compatibility with the testing library. Store testing is recommended for most use cases.

## Backend Testing

- **Framework**: Built-in Rust test framework
- **Location**: Test modules are defined with `#[cfg(test)]` at the end of implementation files

### Example: Unit Tests

See:
- `src-tauri/src/commands/chat.rs` - Session management tests
- `src-tauri/src/claude/permissions.rs` - Permission logic tests

## Current Test Coverage

### Frontend
- ✅ Chat store: message management, streaming, activities, queue
- 📝 Additional store tests can be added following the chat.test.ts pattern

### Backend
- ✅ Placeholder tests for session creation and permissions
- 📝 Tests can be expanded to cover actual implementation logic

## Adding New Tests

### Frontend Store Test
Create a file `<store-name>.test.ts` next to your store file:

```typescript
import { describe, it, expect, beforeEach } from 'vitest';
import { myStore } from './my-store';
import { get } from 'svelte/store';

describe('myStore', () => {
  beforeEach(() => {
    myStore.reset();
  });

  it('should do something', () => {
    myStore.doSomething();
    expect(get(myStore).value).toBe('expected');
  });
});
```

### Rust Test
Add a test module at the end of your Rust file:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_something() {
        assert_eq!(2 + 2, 4);
    }
}
```
