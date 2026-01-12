// Test file for Sovereign VS Code Extension
// Try these features:
// 1. Select code and right-click -> "Sovereign: Explain Selection"
// 2. Select code and right-click -> "Sovereign: Review Code"
// 3. Cmd+Shift+P -> "Sovereign: Generate Code"

// Sample function with a bug - try "Fix Bug" on this
function calculateAverage(numbers: number[]): number {
    let sum = 0;
    for (let i = 0; i <= numbers.length; i++) {  // Bug: should be < not <=
        sum += numbers[i];
    }
    return sum / numbers.length;
}

// Sample function to explain
function debounce<T extends (...args: any[]) => any>(
    func: T,
    wait: number
): (...args: Parameters<T>) => void {
    let timeoutId: ReturnType<typeof setTimeout> | null = null;

    return function (this: any, ...args: Parameters<T>) {
        if (timeoutId) {
            clearTimeout(timeoutId);
        }
        timeoutId = setTimeout(() => {
            func.apply(this, args);
        }, wait);
    };
}

// Sample function to generate tests for
function isPalindrome(str: string): boolean {
    const cleaned = str.toLowerCase().replace(/[^a-z0-9]/g, '');
    return cleaned === cleaned.split('').reverse().join('');
}

// Sample class to review
class UserService {
    private users: Map<string, any> = new Map();

    addUser(id: string, data: any) {
        this.users.set(id, data);
    }

    getUser(id: string) {
        return this.users.get(id);
    }

    deleteUser(id: string) {
        this.users.delete(id);
    }
}

export { calculateAverage, debounce, isPalindrome, UserService };
