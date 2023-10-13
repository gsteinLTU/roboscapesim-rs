
(function () {    
    class UseDevServices extends Extension {
        constructor(ide) {
            super('Use Dev Services');
        }

        onOpenRole() {
            world.children[0].services.defaultHost.url = "https://services.dev.netsblox.org";
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

    NetsBloxExtensions.register(UseDevServices);

})();