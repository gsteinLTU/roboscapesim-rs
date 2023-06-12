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
    var s = document.createElement('script');
    s.type = "module";
    s.innerHTML = `import init, {} from '${path}/pkg/netsblox_extension_rs.js';
    
    
        await init();

        window.RoboScapeOnline_fns = {};

        `;
    document.body.appendChild(s);

	var element = document.createElement('link');
	element.setAttribute('rel', 'stylesheet');
	element.setAttribute('type', 'text/css');
	element.setAttribute('href', 'https://gsteinltu.github.io/PseudoMorphic/style.css');
	document.head.appendChild(element);

	var element = document.createElement('script');
	element.setAttribute('src', 'https://gsteinltu.github.io/PseudoMorphic/script.js');
	document.head.appendChild(element);
})();