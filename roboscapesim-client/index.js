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
				Extension.ExtensionSetting.createFromLocalStorage('Beeps Enabled', 'roboscape_beep', true, 'Robots can beep', 'Robots cannot beep', false),
				Extension.ExtensionSetting.createFromLocalStorage('Robot ID Billboards Enabled', 'roboscape_id_billboards', true, 'Robot IDs show over heads', 'Robots IDs hidden', false),

            ];
        }

        getMenu() {
            return {
				'New simulation...': window.RoboScapeOnline_fns.new_room,
				'Show 3D View': window.RoboScapeOnline_fns.show_3d_view,

            };
        }

        getCategories() {
            return [

            ];
        }

        getPalette() {
            return [
				new Extension.PaletteCategory(
					'network',
					[
						new Extension.Palette.Block('robotsInRoom'),
					],
					SpriteMorph
				),
				new Extension.PaletteCategory(
					'network',
					[
						new Extension.Palette.Block('robotsInRoom'),
					],
					StageMorph
				),

            ];
        }

        getBlocks() {
            return [
				new Extension.Block(
					'robotsInRoom',
					'reporter',
					'network',
					'robots in room',
					[],
					function () { return RoboScapeOnline_fns.robots_in_room(); }
				).for(SpriteMorph, StageMorph),

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


    // Add CSS
    var element = document.createElement('link');
    element.setAttribute('rel', 'stylesheet');
    element.setAttribute('type', 'text/css');
    element.setAttribute('href', 'https://gsteinltu.github.io/PseudoMorphic/style.css');
    document.head.appendChild(element);

    // Add JS
    var scriptElement = document.createElement('script');

    scriptElement.onload = () => {        
        // Create 3D view dialog for later use
        {
            var element = createDialog('RoboScape Online');
            element.style.width = '400px';
            element.style.height = '400px';
            const canvas = document.createElement('canvas');
            canvas.id = 'roboscape-canvas';
            canvas.style.width = 'calc(100% - 32px)';
            canvas.style.height = 'calc(100% - 32px)';
            element.querySelector('content').appendChild(canvas);
            setupDialog(element);
            const observer = new ResizeObserver(function () {
                BABYLON.Engine.LastCreatedEngine.resize();
            });
            observer.observe(element);
            window.externalVariables['roboscapedialog'] = element;

            const buttonbar = document.createElement('div');
            buttonbar.id = 'roboscapebuttonbar';
            element.querySelector('content').appendChild(buttonbar);
        }

        // Create join dialog for later use
        {
            var element = createDialog('Join a Session', false, ['Join', 'Close']);
            element.querySelector('content').innerHTML += `
            <div style="margin-bottom: 12px;"><label>Room ID:&nbsp;</label><input class="inset"/></div>
            <div><label>Password:&nbsp;</label><input class="inset"/></div>
            `;

            setupDialog(element, false);
            window.externalVariables['roboscapedialog-join'] = element;

            element = createDialog('Create a Session', false, ['Create', 'Close']);
            element.querySelector('content').innerHTML += `
            <div style="margin-bottom: 12px;"><label>Password:&nbsp;</label><input class="inset"/></div>
            <div><label>Environment:&nbsp;</label><input class="inset"/></div>
            `;

            setupDialog(element, false);
            window.externalVariables['roboscapedialog-new'] = element;
        }
    };

    scriptElement.setAttribute('src', 'https://gsteinltu.github.io/PseudoMorphic/script.js');
    document.head.appendChild(scriptElement);


    var scriptElement = document.createElement('script');
    scriptElement.async = false;

	scriptElement.onload = () => {
		var loaderScriptElement = document.createElement('script');
		loaderScriptElement.async = false;
		loaderScriptElement.onload = () => {
		    var s = document.createElement('script');
		    s.type = "module";
		    s.innerHTML = `import init, {robots_in_room, new_room, show_3d_view} from '${path}/pkg/roboscapesim_client.js';
		    
		    
		        await init();
		
		        window.RoboScapeOnline_fns = {};
				window.RoboScapeOnline_fns.robots_in_room = robots_in_room;
				window.RoboScapeOnline_fns.new_room = new_room;
				window.RoboScapeOnline_fns.show_3d_view = show_3d_view;
		        `;
		    document.body.appendChild(s);
		};
		loaderScriptElement.setAttribute('src', 'https://preview.babylonjs.com/loaders/babylonjs.loaders.min.js');
		document.head.appendChild(loaderScriptElement);
	};
	scriptElement.setAttribute('src', 'https://preview.babylonjs.com/babylon.js');
	document.head.appendChild(scriptElement);
	disableRetinaSupport();
})();