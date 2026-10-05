// Runtime observation is injected only into test responses, never production assets.
export const capture = `<script>
window.testOutput = '';
window.testErrors = [];
window.testEvents = [];
window.testAlerts = [];
window.alert = message => window.testAlerts.push(String(message));
window.addEventListener('error', event => window.testErrors.push(event.message));
window.addEventListener('unhandledrejection', event => window.testErrors.push(String(event.reason)));
const instantiate = Riscbox.instantiate.bind(Riscbox);
Riscbox.instantiate = async (bytes, options) => {
    const runtime = await instantiate(bytes, { ...options,
        consoleWrite: text => { window.testOutput += text; options.consoleWrite?.(text); },
        onVmReset: cause => { window.testEvents.push(cause); options.onVmReset?.(cause); },
        onError: error => { window.testErrors.push(String(error)); options.onError?.(error); },
    });
    window.testRuntime = runtime;
    return runtime;
};
</script>`;

export function observeApp(path, bytes) {
    return path.endsWith('index.html')
        ? Buffer.from(bytes.toString().replace(/(<script src="riscbox-[^"]+\/riscbox.js"><\/script>)/, `$1${capture}`))
        : bytes;
}
