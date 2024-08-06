window.addEventListener('load', function () {
	const scanModal = document.getElementById("scanModal")

	const scanVideo = document.getElementById("scanVideo");

	const isbnModalForm = document.getElementById("isbnModalForm");

	try {
		window['BarcodeDetector'].getSupportedFormats()
	} catch {
		window['BarcodeDetector'] = barcodeDetectorPolyfill.BarcodeDetectorPolyfill
	}

	const barcodeDetector = new BarcodeDetector({formats: ['isbn_13']});

	let stream = null;
	let barcodeInterval = null;

	scanModal.addEventListener('show.bs.modal', async () => {
		stream = await navigator.mediaDevices.getUserMedia({
			video: {
				facingMode: { ideal: 'environment' }
			},
			audio: false
		});
		scanVideo.srcObject = stream
		await scanVideo.play()

		barcodeInterval = window.setInterval(async () => {
			const barcodes = await barcodeDetector.detect(scanVideo);
			if (barcodes.length <= 0) return;
			
			var searchParams = new URLSearchParams(window.location.search);
			searchParams.set("isbn", barcodes[0].rawValue);
			searchParams.set("provider", isbnModalForm.provider.value)
			window.location.search = searchParams.toString();

			bootstrap.Modal.getInstance("#scanModal").hide()
		}, 200);

		console.log('Reading barcodes.')
	})

	scanModal.addEventListener('hidden.bs.modal', () => {
		if (barcodeInterval !== null) {
			window.clearInterval(barcodeInterval);
			barcodeInterval = null;
		}

		if (stream !== null) {
			stream.getTracks().forEach(function(track) {
				track.stop();
			});
			stream = null
		}

		console.log('Reset.')
	})
})
