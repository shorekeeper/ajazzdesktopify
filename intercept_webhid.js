// WebHID traffic interceptor for AK680 MAX protocol reverse-engineering.
// Paste into Edge/Chrome DevTools Console BEFORE connecting the keyboard
// on ajazz.driveall.cn or similar WebHID configuration tools.

(() => {
    const log = [];
    window.__hidLog = log;

    const origRequestDevice = navigator.hid.requestDevice.bind(navigator.hid);
    navigator.hid.requestDevice = async function(opts) {
        console.log('[HID] requestDevice:', opts);
        const devices = await origRequestDevice(opts);
        return devices.map(wrapDevice);
    };

    function wrapDevice(device) {
        const origOpen = device.open.bind(device);
        const origSendReport = device.sendReport.bind(device);
        const origSendFeatureReport = device.sendFeatureReport.bind(device);
        const origReceiveFeatureReport = device.receiveFeatureReport.bind(device);

        device.open = async function() {
            console.log('[HID] open:', device.productName,
                'VID:', hex16(device.vendorId), 'PID:', hex16(device.productId));
            device.collections.forEach((c, i) => {
                console.log(`  collection[${i}]: page=${hex16(c.usagePage)} usage=${hex16(c.usage)}`);
            });
            return origOpen();
        };

        device.sendReport = async function(reportId, data) {
            const arr = new Uint8Array(data);
            const entry = { ts: Date.now(), dir: 'OUT', type: 'report', reportId, data: [...arr] };
            log.push(entry);
            console.log('[HID] sendReport:', reportId, hexDump(arr));
            return origSendReport(reportId, data);
        };

        device.sendFeatureReport = async function(reportId, data) {
            const arr = new Uint8Array(data);
            const entry = { ts: Date.now(), dir: 'OUT', type: 'feature', reportId, data: [...arr] };
            log.push(entry);
            console.log('[HID] sendFeatureReport:', reportId, hexDump(arr));
            return origSendFeatureReport(reportId, data);
        };

        device.receiveFeatureReport = async function(reportId) {
            const result = await origReceiveFeatureReport(reportId);
            const arr = new Uint8Array(result.buffer);
            const entry = { ts: Date.now(), dir: 'IN', type: 'feature', reportId, data: [...arr] };
            log.push(entry);
            console.log('[HID] receiveFeatureReport:', reportId, hexDump(arr));
            return result;
        };

        const origAddEventListener = device.addEventListener.bind(device);
        device.addEventListener = function(type, handler, opts) {
            if (type === 'inputreport') {
                const wrappedHandler = (e) => {
                    const arr = new Uint8Array(e.data.buffer);
                    const entry = { ts: Date.now(), dir: 'IN', type: 'inputreport', reportId: e.reportId, data: [...arr] };
                    log.push(entry);
                    console.log('[HID] inputreport:', hex8(e.reportId), hexDump(arr));
                    handler(e);
                };
                return origAddEventListener(type, wrappedHandler, opts);
            }
            return origAddEventListener(type, handler, opts);
        };

        return device;
    }

    function hex8(n) { return '0x' + (n || 0).toString(16).padStart(2, '0'); }
    function hex16(n) { return '0x' + (n || 0).toString(16).padStart(4, '0'); }
    function hexDump(arr) {
        return [...arr].map(b => b.toString(16).padStart(2, '0')).join(' ');
    }

    window.saveHidLog = () => {
        const json = JSON.stringify(log, null, 2);
        const blob = new Blob([json], { type: 'application/json' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = 'hid_traffic_' + Date.now() + '.json';
        a.click();
        console.log(`Saved ${log.length} packets`);
    };

    window.dumpHidLog = () => {
        log.forEach((e, i) => {
            const hex = e.data.map(b => b.toString(16).padStart(2, '0')).join(' ');
            const prefix = e.dir === 'OUT' ? '>>' : '<<';
            console.log(`[${i}] ${prefix} ${e.type} id=${e.reportId} ${hex}`);
        });
        console.log(`Total: ${log.length} packets`);
    };

    console.log('[HID Interceptor] Ready. Connect keyboard then use:');
    console.log('  dumpHidLog() - print captured traffic');
    console.log('  saveHidLog() - download as JSON file');
})();