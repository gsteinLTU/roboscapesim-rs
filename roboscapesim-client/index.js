/**
 * The following file is generated through a build script. Manually modifying it is an at-your-own-risk activity and your changes will likely be overridden.
 */

(function () {    
    class RoboScapeOnline extends Extension {
        constructor(ide) {
            super('RoboScape Online');
        }

        onOpenRole() {

        }

        getSettings() {
            return [

            ];
        }

        getMenu() {
            return {
				'Show 3D View': window.RoboScapeOnline_fns.show_3d_view,

            };
        }

        getCategories() {
            return [

            ];
        }

        getPalette() {
            return [

            ];
        }

        getBlocks() {
            return [

            ];
        }

        getLabelParts() {
            return [

            ];
        }

    }

    NetsBloxExtensions.register(RoboScapeOnline);
    let path = document.currentScript.src;
    path = path.substring(0, path.lastIndexOf("/"));

	var element = document.createElement('link');
	element.setAttribute('rel', 'stylesheet');
	element.setAttribute('type', 'text/css');
	element.setAttribute('href', 'https://gsteinltu.github.io/PseudoMorphic/style.css');
	document.head.appendChild(element);

	var scriptElement = document.createElement('script');

	scriptElement.onload = () => {
		var element = createDialog('RoboScape Online');
		const canvas = document.createElement('canvas');
		canvas.id = 'roboscape-canvas';
		canvas.style.width = 'calc(100% - 32px)';
		element.querySelector('content').appendChild(canvas);
		setupDialog(element);
		const observer = new ResizeObserver(function () {
		    BABYLON.Engine.LastCreatedEngine.resize();
		});
		observer.observe(element);
		window.externalVariables['roboscapedialog'] = element;
	};
	scriptElement.setAttribute('src', 'https://gsteinltu.github.io/PseudoMorphic/script.js');
	document.head.appendChild(scriptElement);

	var scriptElement = document.createElement('script');
	scriptElement.async = false;

	scriptElement.onload = () => {
		var loaderScriptElement = document.createElement('script');
		loaderScriptElement.async = false;
		loaderScriptElement.setAttribute('src', 'https://preview.babylonjs.com/loaders/babylonjs.loaders.min.js');
		document.head.appendChild(loaderScriptElement);
	    var s = document.createElement('script');
	    s.type = "module";
	    s.innerHTML = `import init, {show_3d_view} from '${path}/pkg/roboscapesim_client.js';
	    
	    
	        await init();
	
	        window.RoboScapeOnline_fns = {};
			window.RoboScapeOnline_fns.show_3d_view = show_3d_view;
	        `;
	    document.body.appendChild(s);
	};
	scriptElement.setAttribute('src', 'https://preview.babylonjs.com/babylon.js');
	document.head.appendChild(scriptElement);
	disableRetinaSupport();
})();